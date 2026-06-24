pub const FRAG_LAST: u32 = 0x8000_0000;
pub const MSG_CALL: u32 = 0;
pub const RPC_VERS: u32 = 2;
pub const NFS_PROG: u32 = 100003;
pub const NFS_VERS: u32 = 3;
pub const AUTH_NONE: u32 = 0;

pub fn push_u32(buf: &mut Vec<u8>, v: u32) {
    buf.extend_from_slice(&v.to_be_bytes());
}

pub fn push_u64(buf: &mut Vec<u8>, v: u64) {
    buf.extend_from_slice(&v.to_be_bytes());
}

pub fn pad(buf: &mut Vec<u8>, n: usize) {
    let p = (4 - (n & 3)) & 3;
    buf.extend(std::iter::repeat_n(0u8, p));
}

pub fn push_opaque(buf: &mut Vec<u8>, bytes: &[u8]) {
    push_u32(buf, bytes.len() as u32);
    buf.extend_from_slice(bytes);
    pad(buf, bytes.len());
}

pub fn build_rpc_frame(proc: u32, xid: u32, args: &[u8]) -> Vec<u8> {
    let mut body = Vec::new();
    push_u32(&mut body, xid);
    push_u32(&mut body, MSG_CALL);
    push_u32(&mut body, RPC_VERS);
    push_u32(&mut body, NFS_PROG);
    push_u32(&mut body, NFS_VERS);
    push_u32(&mut body, proc);
    push_u32(&mut body, AUTH_NONE);
    push_u32(&mut body, 0);
    push_u32(&mut body, AUTH_NONE);
    push_u32(&mut body, 0);
    body.extend_from_slice(args);

    let mut frame = Vec::with_capacity(body.len() + 4);
    push_u32(&mut frame, FRAG_LAST | (body.len() as u32));
    frame.extend_from_slice(&body);
    frame
}
