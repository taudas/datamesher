use std::env;
use std::time::Duration;

use dtmshr_rdma::{net, ConnectionInfo, MemoryRegion, QueuePair, RdmaEndpoint};

const CQ_DEPTH: i32 = 16;
const QP_DEPTH: u32 = 16;
const LOCAL_PSN: u32 = 0;
const DEFAULT_BIND_ADDR: &str = "0.0.0.0:7471";
const POLL_IDLE_SLEEP: Duration = Duration::from_millis(50);

/// Placeholder for the memory this producer exposes as its SSI compute
/// surface. Real sizing/lifetime TBD once the SSI model is decided.
const SSI_BUFFER_LEN: usize = 4096;

/// A single control-channel message buffer. Not the SSI data path — this
/// is just enough two-sided send/receive to prove the connected queue pair
/// actually carries traffic, before any real request/response protocol
/// exists.
const RECV_BUFFER_LEN: usize = 256;

fn main() -> std::io::Result<()> {
    let bind_addr = env::args().nth(1).unwrap_or_else(|| DEFAULT_BIND_ADDR.to_string());

    let endpoint = RdmaEndpoint::open_first_device(CQ_DEPTH)?;
    let qp = QueuePair::create_rc(&endpoint, QP_DEPTH, QP_DEPTH)?;

    let mr = MemoryRegion::register(&endpoint, SSI_BUFFER_LEN)?;

    let local = ConnectionInfo {
        qp_num: qp.qp_num(),
        psn: LOCAL_PSN,
        gid: endpoint.gid(0)?,
        rkey: mr.rkey(),
        addr: mr.addr(),
    };

    eprintln!("dtmshr-producer: qp_num={} rkey={} listening on {bind_addr}", local.qp_num, local.rkey);

    let remote = net::accept_and_exchange(&bind_addr, &local)?;
    qp.connect(LOCAL_PSN, &remote)?;

    eprintln!(
        "dtmshr-producer: connected, remote qp_num={} rkey={}",
        remote.qp_num, remote.rkey
    );

    // TODO: this is just a control-channel receive loop, proving traffic
    // flows over the connected QP. Real SSI compute exposure (what a
    // "request" even means) is still unbuilt.
    let recv_mr = MemoryRegion::register(&endpoint, RECV_BUFFER_LEN)?;
    qp.post_recv(&recv_mr, 0)?;
    eprintln!("dtmshr-producer: control channel up, waiting for messages");

    let mut next_wr_id = 1u64;
    loop {
        let completions = endpoint.poll_cq(QP_DEPTH as usize)?;
        if completions.is_empty() {
            std::thread::sleep(POLL_IDLE_SLEEP);
            continue;
        }

        for wc in completions {
            if wc.status != 0 {
                eprintln!("dtmshr-producer: work completion failed, status={}", wc.status);
                continue;
            }
            let raw = &recv_mr.as_slice()[..wc.byte_len as usize];
            let msg = String::from_utf8_lossy(raw);
            let msg = msg.trim_end_matches('\0');
            eprintln!("dtmshr-producer: received {} bytes: {msg:?}", wc.byte_len);

            qp.post_recv(&recv_mr, next_wr_id)?;
            next_wr_id += 1;
        }
    }
}
