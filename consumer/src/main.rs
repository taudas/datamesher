use dtmshr_rdma::{MemoryRegion, QueuePair, RdmaEndpoint};

const CQ_DEPTH: i32 = 16;
const QP_DEPTH: u32 = 16;

/// Placeholder for the local staging buffer used to shuttle offloaded work
/// to/from a producer's SSI compute node. Real sizing/lifetime TBD.
const OFFLOAD_BUFFER_LEN: usize = 4096;

fn main() -> std::io::Result<()> {
    let endpoint = RdmaEndpoint::open_first_device(CQ_DEPTH)?;
    let qp = QueuePair::create_rc(&endpoint, QP_DEPTH, QP_DEPTH)?;

    let mut offload_buffer = vec![0u8; OFFLOAD_BUFFER_LEN];
    let mr = MemoryRegion::register(&endpoint, &mut offload_buffer)?;

    eprintln!(
        "dtmshr-consumer: qp_num={} rkey={} up, no producer to connect to yet (not implemented)",
        qp.qp_num(),
        mr.rkey(),
    );

    // TODO: discover/select a producer, exchange connection info (qp_num,
    // psn, lid/gid, rkey) out-of-band, modify_qp through RTR -> RTS, then
    // hook into whatever "existing software" is offloading CPU work.

    drop(mr);
    drop(qp);
    drop(endpoint);
    Ok(())
}
