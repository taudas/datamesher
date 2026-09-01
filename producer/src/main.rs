use std::env;
use std::time::Duration;

use dtmshr_rdma::{ibv_wc_opcode, job, net, ConnectionInfo, MemoryRegion, QueuePair, RdmaEndpoint};

const CQ_DEPTH: i32 = 16;
const QP_DEPTH: u32 = 16;
const LOCAL_PSN: u32 = 0;
const DEFAULT_BIND_ADDR: &str = "0.0.0.0:7471";
const POLL_IDLE_SLEEP: Duration = Duration::from_millis(50);

/// Placeholder for the memory this producer exposes as its SSI compute
/// surface. Real sizing/lifetime TBD once the SSI model is decided. Unused
/// by the job protocol below -- that's a separate, narrower thing.
const SSI_BUFFER_LEN: usize = 4096;

/// Largest job input/result this producer will handle. Fixed and tiny —
/// this is a protocol smoke test, not a real workload's sizing.
const MAX_JOB_LEN: usize = 64;
const REQUEST_BUFFER_LEN: usize = job::HEADER_LEN + MAX_JOB_LEN;

/// wr_id for post_recv/post_send calls where the id carries no meaning of
/// its own (unlike the RDMA WRITE below, where wr_id *is* the job_id).
const OPAQUE_WR_ID: u64 = 0;

fn main() -> std::io::Result<()> {
    let bind_addr = env::args()
        .nth(1)
        .unwrap_or_else(|| DEFAULT_BIND_ADDR.to_string());

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

    eprintln!(
        "dtmshr-producer: qp_num={} rkey={} listening on {bind_addr}",
        local.qp_num, local.rkey
    );

    let remote = net::accept_and_exchange(&bind_addr, &local)?;
    qp.connect(LOCAL_PSN, &remote)?;

    eprintln!(
        "dtmshr-producer: connected, remote qp_num={} rkey={}",
        remote.qp_num, remote.rkey
    );

    // Job/RPC channel: consumer sends a JobRequest, we execute it and RDMA
    // WRITE the result into the buffer it named, then send a JobDone
    // notice (a plain RDMA WRITE doesn't tell the receiver it landed).
    let request_mr = MemoryRegion::register(&endpoint, REQUEST_BUFFER_LEN)?;
    qp.post_recv(&request_mr, OPAQUE_WR_ID)?;

    // Reused across jobs -- overwritten before every RDMA WRITE, sized to
    // match the consumer's result buffer (see consumer/src/main.rs).
    let mut result_scratch = MemoryRegion::register(&endpoint, MAX_JOB_LEN)?;
    let mut notify_mr = MemoryRegion::register(&endpoint, job::DONE_LEN)?;

    eprintln!("dtmshr-producer: ready to serve jobs");

    loop {
        let completions = endpoint.poll_cq(QP_DEPTH as usize)?;
        if completions.is_empty() {
            std::thread::sleep(POLL_IDLE_SLEEP);
            continue;
        }

        for wc in completions {
            if wc.status != 0 {
                eprintln!(
                    "dtmshr-producer: work completion failed, status={}",
                    wc.status
                );
                continue;
            }

            if wc.opcode == ibv_wc_opcode::IBV_WC_RECV {
                let raw = &request_mr.as_slice()[..wc.byte_len as usize];
                match job::JobRequest::decode(raw) {
                    Ok((request, input)) => match job::execute(request.opcode, input) {
                        Ok(result) => {
                            result_scratch.as_mut_slice()[..result.len()].copy_from_slice(&result);
                            qp.post_rdma_write(
                                &result_scratch,
                                request.result_addr,
                                request.result_rkey,
                                request.job_id,
                            )?;
                            eprintln!(
                                "dtmshr-producer: job {} executed ({} -> {} bytes), RDMA WRITE posted",
                                request.job_id,
                                input.len(),
                                result.len()
                            );
                        }
                        Err(e) => eprintln!("dtmshr-producer: job {} failed: {e}", request.job_id),
                    },
                    Err(e) => eprintln!("dtmshr-producer: bad job request: {e}"),
                }
                qp.post_recv(&request_mr, OPAQUE_WR_ID)?;
            } else if wc.opcode == ibv_wc_opcode::IBV_WC_RDMA_WRITE {
                // wr_id was set to the job_id in post_rdma_write above.
                let job_id = wc.wr_id;
                job::JobDone { job_id }.encode(notify_mr.as_mut_slice())?;
                qp.post_send(&notify_mr, OPAQUE_WR_ID)?;
                eprintln!("dtmshr-producer: job {job_id} result delivered, notified consumer");
            }
            // IBV_WC_SEND (the notify itself completing): nothing to do.
        }
    }
}
