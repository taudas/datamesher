use std::env;
use std::process::ExitCode;
use std::time::Duration;

use dtmshr_rdma::{net, ConnectionInfo, MemoryRegion, QueuePair, RdmaEndpoint};

const CQ_DEPTH: i32 = 16;
const QP_DEPTH: u32 = 16;
const LOCAL_PSN: u32 = 0;
const PING_INTERVAL: Duration = Duration::from_secs(5);

/// Placeholder for the local staging buffer used to shuttle offloaded work
/// to/from a producer's SSI compute node. Real sizing/lifetime TBD.
const OFFLOAD_BUFFER_LEN: usize = 4096;

/// Matches the producer's control-channel buffer size. Not the SSI data
/// path — just enough to prove the connected queue pair carries traffic.
const SEND_BUFFER_LEN: usize = 256;

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

    // TODO: workload interception/offload API — this is just a heartbeat
    // proving the connected QP carries traffic, not a real offload path.
    let mut send_mr = MemoryRegion::register(&endpoint, SEND_BUFFER_LEN)?;
    let mut ping_count = 0u64;
    loop {
        // Drain send completions so the CQ doesn't fill up over time —
        // post_send is signaled, and an unpolled CQ eventually overflows.
        for wc in endpoint.poll_cq(QP_DEPTH as usize)? {
            if wc.status != 0 {
                eprintln!("dtmshr-consumer: send failed, status={}", wc.status);
            }
        }

        ping_count += 1;
        let message = format!("ping {ping_count}");
        let buf = send_mr.as_mut_slice();
        buf.fill(0);
        buf[..message.len()].copy_from_slice(message.as_bytes());

        qp.post_send(&send_mr, ping_count)?;
        eprintln!("dtmshr-consumer: sent {message:?}");

        std::thread::sleep(PING_INTERVAL);
    }
}
