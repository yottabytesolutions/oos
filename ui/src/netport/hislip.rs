// SPDX-License-Identifier: GPL-2.0-or-later
//! Runtime-neutral HiSLIP message framing and limits.
pub const PORT: u16 = 4880;
pub(crate) const HEADER: usize = 16;
/// Same ceiling as one SCPI line (`:SOURce:ARB:DATA`).
pub(crate) const MAX_PAYLOAD: usize = 4 << 20;
/// Smallest reply payload of one message, whatever AsyncMaximumMessageSize asks: a smaller one only multiplies the
/// messages (one byte each made a message per byte of the reply).
pub(crate) const MIN_PAYLOAD: usize = 4 << 10;
/// Eight existing response assemblers may each grow geometrically up to twice the SCPI reply limit.
pub(crate) const REPLY_BYTES: usize = 2 * super::RESPONSE_BYTES + 4096;
/// Protocol 1.0. 2.0 (secure connection) is not spoken, so the negotiated version stays 1.0.
pub(crate) const VERSION: u16 = 0x0100;

pub(crate) const INITIALIZE: u8 = 0;
pub(crate) const INITIALIZE_RESPONSE: u8 = 1;
pub(crate) const FATAL_ERROR: u8 = 2;
pub(crate) const ERROR: u8 = 3;
pub(crate) const ASYNC_LOCK: u8 = 4;
pub(crate) const ASYNC_LOCK_RESPONSE: u8 = 5;
pub(crate) const DATA: u8 = 6;
pub(crate) const DATA_END: u8 = 7;
pub(crate) const DEVICE_CLEAR_COMPLETE: u8 = 8;
pub(crate) const DEVICE_CLEAR_ACKNOWLEDGE: u8 = 9;
pub(crate) const ASYNC_REMOTE_LOCAL_CONTROL: u8 = 10;
pub(crate) const ASYNC_REMOTE_LOCAL_RESPONSE: u8 = 11;
pub(crate) const ASYNC_MAXIMUM_MESSAGE_SIZE: u8 = 15;
pub(crate) const ASYNC_MAXIMUM_MESSAGE_SIZE_RESPONSE: u8 = 16;
pub(crate) const ASYNC_INITIALIZE: u8 = 17;
pub(crate) const ASYNC_INITIALIZE_RESPONSE: u8 = 18;
pub(crate) const ASYNC_DEVICE_CLEAR: u8 = 19;
pub(crate) const ASYNC_STATUS_QUERY: u8 = 21;
pub(crate) const ASYNC_STATUS_RESPONSE: u8 = 22;
pub(crate) const ASYNC_DEVICE_CLEAR_ACKNOWLEDGE: u8 = 23;
pub(crate) const ASYNC_LOCK_INFO: u8 = 24;
pub(crate) const ASYNC_LOCK_INFO_RESPONSE: u8 = 25;

pub(crate) const FATAL_BAD_HEADER: u8 = 1;
pub(crate) const FATAL_BAD_INIT: u8 = 3;
pub(crate) const ERR_TYPE: u8 = 1;
pub(crate) const ERR_LARGE: u8 = 4;

/// One HiSLIP message. `param` is the 32-bit message parameter.
#[derive(Clone, Debug, PartialEq)]
pub struct Message {
    pub typ: u8,
    pub control: u8,
    pub param: u32,
    pub data: Vec<u8>,
}

pub(crate) fn header(typ: u8, control: u8, param: u32, length: usize) -> [u8; HEADER] {
    let mut header = [0; HEADER];
    header[..2].copy_from_slice(b"HS");
    header[2] = typ;
    header[3] = control;
    header[4..8].copy_from_slice(&param.to_be_bytes());
    header[8..].copy_from_slice(&(length as u64).to_be_bytes());
    header
}
pub fn encode(typ: u8, control: u8, param: u32, data: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(HEADER + data.len());
    output.extend_from_slice(&header(typ, control, param, data.len()));
    output.extend_from_slice(data);
    output
}

/// Takes one message from `buf`. `Err` is a header that cannot be a message (the connection should close).
pub fn take_message(buf: &mut Vec<u8>) -> Result<Option<Message>, ()> {
    if buf.len() < HEADER {
        return Ok(None);
    }
    if &buf[..2] != b"HS" {
        return Err(());
    }
    let n = u64::from_be_bytes(buf[8..16].try_into().map_err(|_| ())?);
    if n > MAX_PAYLOAD as u64 {
        return Err(());
    }
    let n = n as usize;
    if buf.len() < HEADER + n {
        return Ok(None);
    }
    let msg = Message {
        typ: buf[2],
        control: buf[3],
        param: u32::from_be_bytes(buf[4..8].try_into().map_err(|_| ())?),
        data: buf[HEADER..HEADER + n].to_vec(),
    };
    buf.drain(..HEADER + n);
    Ok(Some(msg))
}
