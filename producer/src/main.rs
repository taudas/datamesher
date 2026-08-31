use std::env;

use dtmshr_rdma::{net, ConnectionInfo, MemoryRegion, QueuePair, RdmaEndpoint};

const CQ_DEPTH: i32 = 16;
const QP_DEPTH: u32 = 16;
const LOCAL_PSN: u32 = 0;
const DEFAULT_BIND_ADDR: &str = "0.0.0.0:7471";

/// Placeholder for the memory this producer exposes as its SSI compute
/// surface. Real sizing/lifetime TBD once the SSI model is decided.
const SSI_BUFFER_LEN: usize = 4096;

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

    // TODO: SSI compute exposure — right now the queue pair is up and RTS
    // but nothing services it. Park here until that exists.
    loop {
        std::thread::sleep(std::time::Duration::from_secs(60));
    }
}
