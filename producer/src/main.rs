use dtmshr_rdma::{MemoryRegion, QueuePair, RdmaEndpoint};

const CQ_DEPTH: i32 = 16;
const QP_DEPTH: u32 = 16;

/// Placeholder for the memory this producer exposes as its SSI compute
/// surface. Real sizing/lifetime TBD once the SSI model is decided.
const SSI_BUFFER_LEN: usize = 4096;

fn main() -> std::io::Result<()> {
    let endpoint = RdmaEndpoint::open_first_device(CQ_DEPTH)?;
    let qp = QueuePair::create_rc(&endpoint, QP_DEPTH, QP_DEPTH)?;

    let mut ssi_buffer = vec![0u8; SSI_BUFFER_LEN];
    let mr = MemoryRegion::register(&endpoint, &mut ssi_buffer)?;

    eprintln!(
        "dtmshr-producer: qp_num={} rkey={} up, waiting for consumer connection info (not implemented yet)",
        qp.qp_num(),
        mr.rkey(),
    );

    // TODO: listen for a consumer's out-of-band connection request (qp_num,
    // psn, lid/gid, rkey), exchange this node's own, then modify_qp through
    // RTR -> RTS to actually connect.

    drop(mr);
    drop(qp);
    drop(endpoint);
    Ok(())
}
