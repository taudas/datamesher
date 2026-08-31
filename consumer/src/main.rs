use std::env;
use std::process::ExitCode;

use dtmshr_rdma::{net, ConnectionInfo, MemoryRegion, QueuePair, RdmaEndpoint};

const CQ_DEPTH: i32 = 16;
const QP_DEPTH: u32 = 16;
const LOCAL_PSN: u32 = 0;

/// Placeholder for the local staging buffer used to shuttle offloaded work
/// to/from a producer's SSI compute node. Real sizing/lifetime TBD.
const OFFLOAD_BUFFER_LEN: usize = 4096;

fn main() -> ExitCode {
    let Some(producer_addr) = env::args().nth(1) else {
        eprintln!("usage: dtmshr-consumer <producer-host>:<port>");
        return ExitCode::FAILURE;
    };

    match run(&producer_addr) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("dtmshr-consumer: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(producer_addr: &str) -> std::io::Result<()> {
    let endpoint = RdmaEndpoint::open_first_device(CQ_DEPTH)?;
    let qp = QueuePair::create_rc(&endpoint, QP_DEPTH, QP_DEPTH)?;
    let mr = MemoryRegion::register(&endpoint, OFFLOAD_BUFFER_LEN)?;

    let local = ConnectionInfo {
        qp_num: qp.qp_num(),
        psn: LOCAL_PSN,
        gid: endpoint.gid(0)?,
        rkey: mr.rkey(),
        addr: mr.addr(),
    };

    eprintln!("dtmshr-consumer: qp_num={} rkey={} connecting to {producer_addr}", local.qp_num, local.rkey);

    let remote = net::connect_and_exchange(producer_addr, &local)?;
    qp.connect(LOCAL_PSN, &remote)?;

    eprintln!(
        "dtmshr-consumer: connected to producer qp_num={} rkey={}",
        remote.qp_num, remote.rkey
    );

    // TODO: workload interception/offload API — right now the queue pair is
    // up and RTS but nothing uses it. Park here until that exists.
    loop {
        std::thread::sleep(std::time::Duration::from_secs(60));
    }
}
