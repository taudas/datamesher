use std::env;
use std::process::ExitCode;
use std::time::Duration;

use dtmshr_rdma::{ibv_wc_opcode, job, net, ConnectionInfo, MemoryRegion, QueuePair, RdmaEndpoint};

const CQ_DEPTH: i32 = 16;
const QP_DEPTH: u32 = 16;
const LOCAL_PSN: u32 = 0;
const JOB_INTERVAL: Duration = Duration::from_secs(5);
const POLL_IDLE_SLEEP: Duration = Duration::from_millis(50);

/// Placeholder for the local staging buffer used to shuttle offloaded work
/// to/from a producer's SSI compute node. Real sizing/lifetime TBD. Unused
/// by the job protocol below -- that's a separate, narrower thing.
const OFFLOAD_BUFFER_LEN: usize = 4096;

/// Must match the producer's MAX_JOB_LEN.
const MAX_JOB_LEN: usize = 64;
const REQUEST_BUFFER_LEN: usize = job::HEADER_LEN + MAX_JOB_LEN;

/// wr_id for calls where the id carries no meaning of its own.
const OPAQUE_WR_ID: u64 = 0;

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

    eprintln!(
        "dtmshr-consumer: qp_num={} rkey={} connecting to {producer_addr}",
        local.qp_num, local.rkey
    );

    let remote = net::connect_and_exchange(producer_addr, &local)?;
    qp.connect(LOCAL_PSN, &remote)?;

    eprintln!(
        "dtmshr-consumer: connected to producer qp_num={} rkey={}",
        remote.qp_num, remote.rkey
    );

    // Job/RPC channel: we send a JobRequest naming this result buffer, the
    // producer RDMA WRITEs its answer straight into it (our CPU/QP does
    // nothing for that step), then sends a JobDone notice once it's landed.
    let result_mr = MemoryRegion::register(&endpoint, MAX_JOB_LEN)?;
    let mut request_mr = MemoryRegion::register(&endpoint, REQUEST_BUFFER_LEN)?;
    let notify_mr = MemoryRegion::register(&endpoint, job::DONE_LEN)?;
    qp.post_recv(&notify_mr, OPAQUE_WR_ID)?;

    let mut job_id = 0u64;
    loop {
        job_id += 1;
        let input = format!("job {job_id} from consumer");
        let input = input.as_bytes();
        assert!(
            input.len() <= MAX_JOB_LEN,
            "sample job input exceeds MAX_JOB_LEN"
        );

        let request = job::JobRequest {
            opcode: job::OP_UPPERCASE,
            job_id,
            result_addr: result_mr.addr(),
            result_rkey: result_mr.rkey(),
        };
        request.encode(input, request_mr.as_mut_slice())?;
        qp.post_send(&request_mr, OPAQUE_WR_ID)?;
        eprintln!(
            "dtmshr-consumer: sent job {job_id}: {:?}",
            String::from_utf8_lossy(input)
        );

        wait_for_job_done(&endpoint, &qp, &notify_mr, job_id)?;

        let result = String::from_utf8_lossy(&result_mr.as_slice()[..input.len()]);
        eprintln!("dtmshr-consumer: job {job_id} result: {result:?}");

        std::thread::sleep(JOB_INTERVAL);
    }
}

/// Polls until the producer's `JobDone` notice for `job_id` arrives,
/// reposting the receive buffer for the next one. Also drains our own
/// `post_send` completions along the way so the CQ doesn't fill up.
fn wait_for_job_done(
    endpoint: &RdmaEndpoint,
    qp: &QueuePair,
    notify_mr: &MemoryRegion,
    job_id: u64,
) -> std::io::Result<()> {
    loop {
        let completions = endpoint.poll_cq(QP_DEPTH as usize)?;
        if completions.is_empty() {
            std::thread::sleep(POLL_IDLE_SLEEP);
            continue;
        }

        for wc in completions {
            if wc.status != 0 {
                eprintln!(
                    "dtmshr-consumer: work completion failed, status={}",
                    wc.status
                );
                continue;
            }
            if wc.opcode == ibv_wc_opcode::IBV_WC_RECV {
                let done = job::JobDone::decode(&notify_mr.as_slice()[..wc.byte_len as usize])?;
                qp.post_recv(notify_mr, OPAQUE_WR_ID)?;
                if done.job_id == job_id {
                    return Ok(());
                }
                eprintln!(
                    "dtmshr-consumer: got JobDone for {} while waiting on {job_id}, ignoring",
                    done.job_id
                );
            }
            // IBV_WC_SEND (our own request completing): nothing to do.
        }
    }
}
