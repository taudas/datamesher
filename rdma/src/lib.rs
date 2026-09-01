//! Shared RDMA bring-up for DTMSHR nodes (producer and consumer sides).
//!
//! Thin safe-ish wrappers over `rdma-sys` (rdma-core/libibverbs FFI).
//! Compiles clean on WSL2 Ubuntu (rdma-core 61.0, vendored `rdma-sys` with a
//! bumped `bindgen` — see `../vendor/rdma-sys/README.md`); not yet run
//! against a real or soft-RoCE device.
//!
//! Enum-typed struct fields (`qp_state`, `qp_type`, `path_mtu`, ...) use
//! bindgen's `constified_enum_module` output, so constants are qualified by
//! module: `ibv_qp_state::IBV_QPS_INIT`, not a bare `IBV_QPS_INIT`. Flag
//! fields (`ibv_access_flags`, `ibv_qp_attr_mask`, `ibv_send_flags`) are
//! bindgen's `bitfield_enum` newtypes — combine with `|` on the values
//! themselves, or OR the raw `.0` fields for a `const`.

use std::ffi::CStr;
use std::io;
use std::ptr;

use rdma_sys::*;

pub mod job;

/// `WorkCompletion::opcode` is one of these — `ibv_wc_opcode` is a bindgen
/// `constified_enum_module` re-exported here so callers dispatching on
/// completion type don't need to depend on `rdma_sys` directly.
pub use rdma_sys::ibv_wc_opcode;

/// Port 1 is the common case for a single-port RDMA device. Revisit if
/// multi-port devices ever matter here.
pub const PORT_NUM: u8 = 1;

/// One open RDMA device with a protection domain and completion queue.
/// The base a producer or consumer builds queue pairs and memory regions on.
pub struct RdmaEndpoint {
    device_list: *mut *mut ibv_device,
    pub context: *mut ibv_context,
    pub pd: *mut ibv_pd,
    pub cq: *mut ibv_cq,
}

impl RdmaEndpoint {
    /// Opens the first available RDMA device on this host.
    pub fn open_first_device(cq_depth: i32) -> io::Result<Self> {
        unsafe {
            let mut num_devices: i32 = 0;
            let device_list = ibv_get_device_list(&mut num_devices);
            if device_list.is_null() || num_devices == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    "no RDMA devices found (is rdma-core installed and a device present?)",
                ));
            }

            let device = *device_list;
            let name = CStr::from_ptr(ibv_get_device_name(device))
                .to_string_lossy()
                .into_owned();
            eprintln!("dtmshr-rdma: opening device {name}");

            let context = ibv_open_device(device);
            if context.is_null() {
                ibv_free_device_list(device_list);
                return Err(io::Error::last_os_error());
            }

            let pd = ibv_alloc_pd(context);
            if pd.is_null() {
                let err = io::Error::last_os_error();
                ibv_close_device(context);
                ibv_free_device_list(device_list);
                return Err(err);
            }

            let cq = ibv_create_cq(context, cq_depth, ptr::null_mut(), ptr::null_mut(), 0);
            if cq.is_null() {
                let err = io::Error::last_os_error();
                ibv_dealloc_pd(pd);
                ibv_close_device(context);
                ibv_free_device_list(device_list);
                return Err(err);
            }

            Ok(Self {
                device_list,
                context,
                pd,
                cq,
            })
        }
    }
}

impl RdmaEndpoint {
    /// GID at the given index on `PORT_NUM`. RoCE has no LID, so the GID is
    /// what goes into the out-of-band exchange and the address handle.
    pub fn gid(&self, index: i32) -> io::Result<[u8; 16]> {
        unsafe {
            let mut gid: ibv_gid = std::mem::zeroed();
            let ret = ibv_query_gid(self.context, PORT_NUM, index, &mut gid);
            if ret != 0 {
                return Err(io::Error::from_raw_os_error(ret));
            }
            Ok(gid.raw)
        }
    }

    /// Drains up to `max` completions from this endpoint's completion queue.
    /// Non-blocking — returns an empty `Vec` if nothing's ready yet.
    pub fn poll_cq(&self, max: usize) -> io::Result<Vec<WorkCompletion>> {
        unsafe {
            // ibv_wc isn't Clone/Copy (rdma-sys defines it by hand, see
            // vendor/rdma-sys/src/types.rs), so `vec![zeroed(); max]` won't
            // work — push each element instead, so the buffer is genuinely
            // initialized rather than just claimed via set_len.
            let mut wc_buf: Vec<ibv_wc> = Vec::with_capacity(max);
            for _ in 0..max {
                wc_buf.push(std::mem::zeroed());
            }
            let n = ibv_poll_cq(self.cq, max as i32, wc_buf.as_mut_ptr());
            if n < 0 {
                return Err(io::Error::other("ibv_poll_cq failed"));
            }
            Ok(wc_buf[..n as usize]
                .iter()
                .map(|wc| WorkCompletion {
                    wr_id: wc.wr_id,
                    status: wc.status,
                    opcode: wc.opcode,
                    byte_len: wc.byte_len,
                })
                .collect())
        }
    }
}

/// A completed send/receive work request, as reported by `poll_cq`.
/// `status` is `IBV_WC_SUCCESS` (0) on success — anything else is a
/// transport-level failure worth logging, not silently ignoring.
#[derive(Debug, Clone, Copy)]
pub struct WorkCompletion {
    pub wr_id: u64,
    pub status: u32,
    pub opcode: u32,
    pub byte_len: u32,
}

impl Drop for RdmaEndpoint {
    fn drop(&mut self) {
        unsafe {
            ibv_destroy_cq(self.cq);
            ibv_dealloc_pd(self.pd);
            ibv_close_device(self.context);
            ibv_free_device_list(self.device_list);
        }
    }
}

// `ibv_access_flags` is a bindgen bitfield_enum newtype (`ibv_access_flags(pub
// c_uint)`); its `|` impl isn't const, so OR the raw `.0` fields instead.
const QP_ACCESS_FLAGS: u32 = ibv_access_flags::IBV_ACCESS_LOCAL_WRITE.0
    | ibv_access_flags::IBV_ACCESS_REMOTE_READ.0
    | ibv_access_flags::IBV_ACCESS_REMOTE_WRITE.0;

/// An RC (reliable connected) queue pair, brought up to INIT.
///
/// Getting to RTR/RTS needs the remote side's QP number, PSN, and LID/GID —
/// that's an out-of-band exchange (a TCP handshake, most likely) that
/// doesn't exist yet. This is the local half only.
pub struct QueuePair {
    pub qp: *mut ibv_qp,
}

impl QueuePair {
    pub fn create_rc(
        endpoint: &RdmaEndpoint,
        max_send_wr: u32,
        max_recv_wr: u32,
    ) -> io::Result<Self> {
        unsafe {
            let mut qp_init_attr: ibv_qp_init_attr = std::mem::zeroed();
            qp_init_attr.send_cq = endpoint.cq;
            qp_init_attr.recv_cq = endpoint.cq;
            qp_init_attr.qp_type = ibv_qp_type::IBV_QPT_RC;
            qp_init_attr.cap.max_send_wr = max_send_wr;
            qp_init_attr.cap.max_recv_wr = max_recv_wr;
            qp_init_attr.cap.max_send_sge = 1;
            qp_init_attr.cap.max_recv_sge = 1;

            let qp = ibv_create_qp(endpoint.pd, &mut qp_init_attr);
            if qp.is_null() {
                return Err(io::Error::last_os_error());
            }

            let mut attr: ibv_qp_attr = std::mem::zeroed();
            attr.qp_state = ibv_qp_state::IBV_QPS_INIT;
            attr.pkey_index = 0;
            attr.port_num = PORT_NUM;
            attr.qp_access_flags = QP_ACCESS_FLAGS;

            let mask = ibv_qp_attr_mask::IBV_QP_STATE.0
                | ibv_qp_attr_mask::IBV_QP_PKEY_INDEX.0
                | ibv_qp_attr_mask::IBV_QP_PORT.0
                | ibv_qp_attr_mask::IBV_QP_ACCESS_FLAGS.0;

            let ret = ibv_modify_qp(qp, &mut attr, mask as i32);
            if ret != 0 {
                ibv_destroy_qp(qp);
                return Err(io::Error::from_raw_os_error(ret));
            }

            Ok(Self { qp })
        }
    }

    pub fn qp_num(&self) -> u32 {
        unsafe { (*self.qp).qp_num }
    }

    /// Drives this queue pair from `INIT` through `RTR` to `RTS` using the
    /// remote side's connection info obtained via out-of-band exchange
    /// (see `net::accept_and_exchange` / `net::connect_and_exchange`).
    pub fn connect(&self, local_psn: u32, remote: &ConnectionInfo) -> io::Result<()> {
        unsafe {
            let mut attr: ibv_qp_attr = std::mem::zeroed();
            attr.qp_state = ibv_qp_state::IBV_QPS_RTR;
            attr.path_mtu = ibv_mtu::IBV_MTU_1024;
            attr.dest_qp_num = remote.qp_num;
            attr.rq_psn = remote.psn;
            attr.max_dest_rd_atomic = 1;
            attr.min_rnr_timer = 12;
            attr.ah_attr.is_global = 1;
            attr.ah_attr.grh.dgid.raw = remote.gid;
            attr.ah_attr.grh.sgid_index = 0;
            attr.ah_attr.grh.hop_limit = 1;
            attr.ah_attr.port_num = PORT_NUM;

            let mask = ibv_qp_attr_mask::IBV_QP_STATE.0
                | ibv_qp_attr_mask::IBV_QP_AV.0
                | ibv_qp_attr_mask::IBV_QP_PATH_MTU.0
                | ibv_qp_attr_mask::IBV_QP_DEST_QPN.0
                | ibv_qp_attr_mask::IBV_QP_RQ_PSN.0
                | ibv_qp_attr_mask::IBV_QP_MAX_DEST_RD_ATOMIC.0
                | ibv_qp_attr_mask::IBV_QP_MIN_RNR_TIMER.0;
            let ret = ibv_modify_qp(self.qp, &mut attr, mask as i32);
            if ret != 0 {
                return Err(io::Error::from_raw_os_error(ret));
            }

            let mut attr: ibv_qp_attr = std::mem::zeroed();
            attr.qp_state = ibv_qp_state::IBV_QPS_RTS;
            attr.timeout = 14;
            attr.retry_cnt = 7;
            attr.rnr_retry = 7;
            attr.sq_psn = local_psn;
            attr.max_rd_atomic = 1;

            let mask = ibv_qp_attr_mask::IBV_QP_STATE.0
                | ibv_qp_attr_mask::IBV_QP_TIMEOUT.0
                | ibv_qp_attr_mask::IBV_QP_RETRY_CNT.0
                | ibv_qp_attr_mask::IBV_QP_RNR_RETRY.0
                | ibv_qp_attr_mask::IBV_QP_SQ_PSN.0
                | ibv_qp_attr_mask::IBV_QP_MAX_QP_RD_ATOMIC.0;
            let ret = ibv_modify_qp(self.qp, &mut attr, mask as i32);
            if ret != 0 {
                return Err(io::Error::from_raw_os_error(ret));
            }

            Ok(())
        }
    }

    /// Posts the whole memory region as one receive buffer, tagged with
    /// `wr_id` so the matching completion (from `RdmaEndpoint::poll_cq`)
    /// can be matched back to it.
    pub fn post_recv(&self, mr: &MemoryRegion, wr_id: u64) -> io::Result<()> {
        unsafe {
            let mut sge = ibv_sge {
                addr: mr.addr(),
                length: mr.len() as u32,
                lkey: mr.lkey(),
            };
            let mut wr: ibv_recv_wr = std::mem::zeroed();
            wr.wr_id = wr_id;
            wr.sg_list = &mut sge;
            wr.num_sge = 1;

            let mut bad_wr: *mut ibv_recv_wr = ptr::null_mut();
            let ret = ibv_post_recv(self.qp, &mut wr, &mut bad_wr);
            if ret != 0 {
                return Err(io::Error::from_raw_os_error(ret));
            }
            Ok(())
        }
    }

    /// Sends the whole memory region as one message (two-sided send —
    /// the peer must have a matching `post_recv` outstanding).
    pub fn post_send(&self, mr: &MemoryRegion, wr_id: u64) -> io::Result<()> {
        unsafe {
            let mut sge = ibv_sge {
                addr: mr.addr(),
                length: mr.len() as u32,
                lkey: mr.lkey(),
            };
            let mut wr: ibv_send_wr = std::mem::zeroed();
            wr.wr_id = wr_id;
            wr.sg_list = &mut sge;
            wr.num_sge = 1;
            wr.opcode = ibv_wr_opcode::IBV_WR_SEND;
            wr.send_flags = ibv_send_flags::IBV_SEND_SIGNALED.0;

            let mut bad_wr: *mut ibv_send_wr = ptr::null_mut();
            let ret = ibv_post_send(self.qp, &mut wr, &mut bad_wr);
            if ret != 0 {
                return Err(io::Error::from_raw_os_error(ret));
            }
            Ok(())
        }
    }

    /// One-sided RDMA WRITE of the whole local memory region into
    /// `remote_addr`/`remote_rkey` on the peer — no action needed on the
    /// peer's side, that's the point of RDMA. The peer isn't notified when
    /// this lands; pair it with a `post_send` if it needs to know.
    pub fn post_rdma_write(
        &self,
        local_mr: &MemoryRegion,
        remote_addr: u64,
        remote_rkey: u32,
        wr_id: u64,
    ) -> io::Result<()> {
        unsafe {
            let mut sge = ibv_sge {
                addr: local_mr.addr(),
                length: local_mr.len() as u32,
                lkey: local_mr.lkey(),
            };
            let mut wr: ibv_send_wr = std::mem::zeroed();
            wr.wr_id = wr_id;
            wr.sg_list = &mut sge;
            wr.num_sge = 1;
            wr.opcode = ibv_wr_opcode::IBV_WR_RDMA_WRITE;
            wr.send_flags = ibv_send_flags::IBV_SEND_SIGNALED.0;
            wr.wr.rdma = rdma_t {
                remote_addr,
                rkey: remote_rkey,
            };

            let mut bad_wr: *mut ibv_send_wr = ptr::null_mut();
            let ret = ibv_post_send(self.qp, &mut wr, &mut bad_wr);
            if ret != 0 {
                return Err(io::Error::from_raw_os_error(ret));
            }
            Ok(())
        }
    }
}

impl Drop for QueuePair {
    fn drop(&mut self) {
        unsafe {
            ibv_destroy_qp(self.qp);
        }
    }
}

/// A registered, pinned memory region the other side can RDMA read/write
/// into, given the rkey. Owns its backing buffer — `ibv_reg_mr` pins the
/// address it's given, so that address must not move or be freed for as
/// long as the registration is live. Field order matters: `mr` is declared
/// before `buf` so `Drop` tears down in that order (deregister, then free).
pub struct MemoryRegion {
    mr: *mut ibv_mr,
    buf: Vec<u8>,
}

impl MemoryRegion {
    pub fn register(endpoint: &RdmaEndpoint, len: usize) -> io::Result<Self> {
        let mut buf = vec![0u8; len];
        unsafe {
            let mr = ibv_reg_mr(
                endpoint.pd,
                buf.as_mut_ptr() as *mut _,
                buf.len(),
                QP_ACCESS_FLAGS as i32,
            );
            if mr.is_null() {
                return Err(io::Error::last_os_error());
            }
            Ok(Self { mr, buf })
        }
    }

    pub fn addr(&self) -> u64 {
        self.buf.as_ptr() as u64
    }

    pub fn len(&self) -> usize {
        self.buf.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    /// Reads the buffer's current contents locally — for the two-sided
    /// send/receive path, where the local side needs to see what a
    /// `post_recv` just filled in.
    pub fn as_slice(&self) -> &[u8] {
        &self.buf
    }

    /// Writes locally into the buffer before a `post_send` — for the
    /// two-sided send/receive path.
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.buf
    }

    pub fn rkey(&self) -> u32 {
        unsafe { (*self.mr).rkey }
    }

    pub fn lkey(&self) -> u32 {
        unsafe { (*self.mr).lkey }
    }
}

impl Drop for MemoryRegion {
    fn drop(&mut self) {
        unsafe {
            ibv_dereg_mr(self.mr);
        }
    }
}

/// Everything the other side needs to connect to this node's queue pair and
/// RDMA into its memory region. RDMA has no discovery of its own — this has
/// to travel over some other channel first (see `net` below).
#[derive(Debug, Clone, Copy)]
pub struct ConnectionInfo {
    pub qp_num: u32,
    pub psn: u32,
    pub gid: [u8; 16],
    pub rkey: u32,
    pub addr: u64,
}

pub const CONNECTION_INFO_LEN: usize = 4 + 4 + 16 + 4 + 8;

impl ConnectionInfo {
    pub fn to_bytes(&self) -> [u8; CONNECTION_INFO_LEN] {
        let mut buf = [0u8; CONNECTION_INFO_LEN];
        buf[0..4].copy_from_slice(&self.qp_num.to_le_bytes());
        buf[4..8].copy_from_slice(&self.psn.to_le_bytes());
        buf[8..24].copy_from_slice(&self.gid);
        buf[24..28].copy_from_slice(&self.rkey.to_le_bytes());
        buf[28..36].copy_from_slice(&self.addr.to_le_bytes());
        buf
    }

    pub fn from_bytes(buf: &[u8; CONNECTION_INFO_LEN]) -> Self {
        Self {
            qp_num: u32::from_le_bytes(buf[0..4].try_into().unwrap()),
            psn: u32::from_le_bytes(buf[4..8].try_into().unwrap()),
            gid: buf[8..24].try_into().unwrap(),
            rkey: u32::from_le_bytes(buf[24..28].try_into().unwrap()),
            addr: u64::from_le_bytes(buf[28..36].try_into().unwrap()),
        }
    }
}

/// Out-of-band `ConnectionInfo` exchange over plain TCP. Not RDMA itself —
/// just the handshake that has to happen before RDMA can start. Whoever's
/// meant to be the connection's "server" (currently: the producer) accepts;
/// the other side dials in.
pub mod net {
    use super::{ConnectionInfo, CONNECTION_INFO_LEN};
    use std::io::{self, Read, Write};
    use std::net::{TcpListener, TcpStream, ToSocketAddrs};

    /// Accept one TCP connection and exchange connection info over it.
    pub fn accept_and_exchange(
        bind_addr: impl ToSocketAddrs,
        local: &ConnectionInfo,
    ) -> io::Result<ConnectionInfo> {
        let listener = TcpListener::bind(bind_addr)?;
        let (mut stream, peer) = listener.accept()?;
        eprintln!("dtmshr-rdma: peer connected from {peer}");
        exchange(&mut stream, local)
    }

    /// Dial out to a peer and exchange connection info over the connection.
    pub fn connect_and_exchange(
        peer_addr: impl ToSocketAddrs,
        local: &ConnectionInfo,
    ) -> io::Result<ConnectionInfo> {
        let mut stream = TcpStream::connect(peer_addr)?;
        exchange(&mut stream, local)
    }

    fn exchange(stream: &mut TcpStream, local: &ConnectionInfo) -> io::Result<ConnectionInfo> {
        stream.write_all(&local.to_bytes())?;
        let mut buf = [0u8; CONNECTION_INFO_LEN];
        stream.read_exact(&mut buf)?;
        Ok(ConnectionInfo::from_bytes(&buf))
    }
}
