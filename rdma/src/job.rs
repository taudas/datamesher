//! Minimal job/RPC protocol layered on top of the RDMA plumbing in
//! `lib.rs`. Not "single system image" — that's a much bigger, still-open
//! question (see ARCHITECTURE.md). This is remote-exec-as-a-service:
//!
//! 1. Consumer sends a `JobRequest` (two-sided `post_send`) naming an
//!    opcode, inline input, and where its own result buffer lives
//!    (`result_addr`/`result_rkey`).
//! 2. Producer executes the opcode against the input (`execute`), RDMA
//!    WRITEs the result into the consumer's buffer (one-sided, the
//!    consumer's CPU/QP does nothing for this step — that's the point of
//!    RDMA), then sends a `JobDone` notice (two-sided) so the consumer
//!    knows the write landed. A plain RDMA WRITE gives the receiver no
//!    signal on its own.

use std::io;

/// Uppercase-ASCII transform. Not a real workload — picked so the full
/// round trip (request -> producer executes -> RDMA WRITE result ->
/// notify) can be verified without pulling in a crate or opening the
/// arbitrary-code-execution/sandboxing question that a real job type
/// would raise.
pub const OP_UPPERCASE: u32 = 1;

/// opcode(4) + job_id(8) + result_addr(8) + result_rkey(4) + input_len(4)
pub const HEADER_LEN: usize = 4 + 8 + 8 + 4 + 4;

pub struct JobRequest {
    pub opcode: u32,
    pub job_id: u64,
    pub result_addr: u64,
    pub result_rkey: u32,
}

impl JobRequest {
    /// Encodes the header followed by `input` into `out`, returning the
    /// number of bytes written. `out` must be at least
    /// `HEADER_LEN + input.len()` bytes.
    pub fn encode(&self, input: &[u8], out: &mut [u8]) -> io::Result<usize> {
        let total = HEADER_LEN + input.len();
        if out.len() < total {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "job request buffer too small",
            ));
        }
        out[0..4].copy_from_slice(&self.opcode.to_le_bytes());
        out[4..12].copy_from_slice(&self.job_id.to_le_bytes());
        out[12..20].copy_from_slice(&self.result_addr.to_le_bytes());
        out[20..24].copy_from_slice(&self.result_rkey.to_le_bytes());
        out[24..28].copy_from_slice(&(input.len() as u32).to_le_bytes());
        out[28..total].copy_from_slice(input);
        Ok(total)
    }

    /// Decodes a header and its input slice out of `buf`. `buf` may carry
    /// trailing bytes beyond the encoded message (e.g. a fixed-size
    /// receive buffer) — only `HEADER_LEN + input_len` bytes are read.
    pub fn decode(buf: &[u8]) -> io::Result<(Self, &[u8])> {
        if buf.len() < HEADER_LEN {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "job request shorter than header",
            ));
        }
        let opcode = u32::from_le_bytes(buf[0..4].try_into().unwrap());
        let job_id = u64::from_le_bytes(buf[4..12].try_into().unwrap());
        let result_addr = u64::from_le_bytes(buf[12..20].try_into().unwrap());
        let result_rkey = u32::from_le_bytes(buf[20..24].try_into().unwrap());
        let input_len = u32::from_le_bytes(buf[24..28].try_into().unwrap()) as usize;
        let input_end = HEADER_LEN + input_len;
        if buf.len() < input_end {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "job request input truncated",
            ));
        }
        Ok((
            Self {
                opcode,
                job_id,
                result_addr,
                result_rkey,
            },
            &buf[HEADER_LEN..input_end],
        ))
    }
}

/// Sent by the producer once its RDMA WRITE of the result has completed,
/// so the consumer knows its result buffer is ready to read.
pub struct JobDone {
    pub job_id: u64,
}

pub const DONE_LEN: usize = 8;

impl JobDone {
    pub fn encode(&self, out: &mut [u8]) -> io::Result<()> {
        if out.len() < DONE_LEN {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "job done buffer too small",
            ));
        }
        out[0..DONE_LEN].copy_from_slice(&self.job_id.to_le_bytes());
        Ok(())
    }

    pub fn decode(buf: &[u8]) -> io::Result<Self> {
        if buf.len() < DONE_LEN {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "job done message too short",
            ));
        }
        Ok(Self {
            job_id: u64::from_le_bytes(buf[0..DONE_LEN].try_into().unwrap()),
        })
    }
}

/// Runs a job's opcode against its input, returning the result bytes.
/// Placeholder job set — real workload execution (arbitrary code,
/// sandboxing, resource limits) is a separate design question this
/// doesn't attempt to answer.
pub fn execute(opcode: u32, input: &[u8]) -> io::Result<Vec<u8>> {
    match opcode {
        OP_UPPERCASE => Ok(input.to_ascii_uppercase()),
        other => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("unknown job opcode {other}"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn job_request_round_trips() {
        let request = JobRequest {
            opcode: OP_UPPERCASE,
            job_id: 42,
            result_addr: 0xdead_beef,
            result_rkey: 7,
        };
        let input = b"hello";
        let mut buf = [0u8; HEADER_LEN + 5];
        let written = request.encode(input, &mut buf).unwrap();
        assert_eq!(written, buf.len());

        let (decoded, decoded_input) = JobRequest::decode(&buf).unwrap();
        assert_eq!(decoded.opcode, request.opcode);
        assert_eq!(decoded.job_id, request.job_id);
        assert_eq!(decoded.result_addr, request.result_addr);
        assert_eq!(decoded.result_rkey, request.result_rkey);
        assert_eq!(decoded_input, input);
    }

    #[test]
    fn job_request_decode_ignores_trailing_padding() {
        // Real usage: a fixed-size receive buffer bigger than the message.
        let request = JobRequest {
            opcode: OP_UPPERCASE,
            job_id: 1,
            result_addr: 0,
            result_rkey: 0,
        };
        let mut buf = [0xAAu8; 128];
        request.encode(b"hi", &mut buf).unwrap();

        let (_, input) = JobRequest::decode(&buf).unwrap();
        assert_eq!(input, b"hi");
    }

    #[test]
    fn job_request_decode_rejects_short_header() {
        let buf = [0u8; HEADER_LEN - 1];
        assert!(JobRequest::decode(&buf).is_err());
    }

    #[test]
    fn job_request_decode_rejects_truncated_input() {
        let request = JobRequest {
            opcode: OP_UPPERCASE,
            job_id: 1,
            result_addr: 0,
            result_rkey: 0,
        };
        // Header claims 5 bytes of input but the buffer only has room for
        // the header itself.
        let mut buf = [0u8; HEADER_LEN];
        request.encode(&[0u8; 0], &mut buf).unwrap();
        buf[24..28].copy_from_slice(&5u32.to_le_bytes());
        assert!(JobRequest::decode(&buf).is_err());
    }

    #[test]
    fn job_done_round_trips() {
        let mut buf = [0u8; DONE_LEN];
        JobDone { job_id: 123 }.encode(&mut buf).unwrap();
        let decoded = JobDone::decode(&buf).unwrap();
        assert_eq!(decoded.job_id, 123);
    }

    #[test]
    fn execute_uppercases() {
        assert_eq!(execute(OP_UPPERCASE, b"hello").unwrap(), b"HELLO");
    }

    #[test]
    fn execute_rejects_unknown_opcode() {
        assert!(execute(999, b"hello").is_err());
    }
}
