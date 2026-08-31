//! Shared RDMA bring-up for DTMSHR nodes (producer and consumer sides).
//!
//! Thin safe-ish wrappers over `rdma-sys` (rdma-core/libibverbs FFI).
//! Unverified against a real build: this environment has no Rust toolchain
//! and no Linux/libibverbs to compile against. Treat as a first pass —
//! run `cargo build` on a Linux box with rdma-core installed and fix up
//! whatever bindgen actually named things before relying on it.

use std::ffi::CStr;
use std::io;
use std::ptr;

use rdma_sys::*;

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

const QP_ACCESS_FLAGS: u32 =
    IBV_ACCESS_LOCAL_WRITE | IBV_ACCESS_REMOTE_READ | IBV_ACCESS_REMOTE_WRITE;

/// An RC (reliable connected) queue pair, brought up to INIT.
///
/// Getting to RTR/RTS needs the remote side's QP number, PSN, and LID/GID —
/// that's an out-of-band exchange (a TCP handshake, most likely) that
/// doesn't exist yet. This is the local half only.
pub struct QueuePair {
    pub qp: *mut ibv_qp,
}

impl QueuePair {
    pub fn create_rc(endpoint: &RdmaEndpoint, max_send_wr: u32, max_recv_wr: u32) -> io::Result<Self> {
        unsafe {
            let mut qp_init_attr: ibv_qp_init_attr = std::mem::zeroed();
            qp_init_attr.send_cq = endpoint.cq;
            qp_init_attr.recv_cq = endpoint.cq;
            qp_init_attr.qp_type = IBV_QPT_RC;
            qp_init_attr.cap.max_send_wr = max_send_wr;
            qp_init_attr.cap.max_recv_wr = max_recv_wr;
            qp_init_attr.cap.max_send_sge = 1;
            qp_init_attr.cap.max_recv_sge = 1;

            let qp = ibv_create_qp(endpoint.pd, &mut qp_init_attr);
            if qp.is_null() {
                return Err(io::Error::last_os_error());
            }

            let mut attr: ibv_qp_attr = std::mem::zeroed();
            attr.qp_state = IBV_QPS_INIT;
            attr.pkey_index = 0;
            attr.port_num = 1;
            attr.qp_access_flags = QP_ACCESS_FLAGS;

            let mask = IBV_QP_STATE | IBV_QP_PKEY_INDEX | IBV_QP_PORT | IBV_QP_ACCESS_FLAGS;

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
}

impl Drop for QueuePair {
    fn drop(&mut self) {
        unsafe {
            ibv_destroy_qp(self.qp);
        }
    }
}

/// A registered, pinned memory region other side can RDMA read/write into,
/// given the rkey.
pub struct MemoryRegion {
    mr: *mut ibv_mr,
}

impl MemoryRegion {
    pub fn register(endpoint: &RdmaEndpoint, buf: &mut [u8]) -> io::Result<Self> {
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
            Ok(Self { mr })
        }
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
