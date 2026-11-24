// SPDX-License-Identifier: GPL-2.0-or-later
//! Response chunks move from interpreter to IO without a second assembled copy.
use super::RESPONSE_BYTES;

pub struct Output {
    chunks: Vec<Chunk>,
    capacity: usize,
    limit: usize,
    hislip: Option<(u32, usize)>,
}
impl Default for Output {
    fn default() -> Self {
        Self::bounded(RESPONSE_BYTES)
    }
}
pub(crate) enum Chunk {
    Owned(Vec<u8>),
    Shared(std::sync::Arc<[u8]>),
}
impl From<Vec<u8>> for Chunk {
    fn from(value: Vec<u8>) -> Self {
        Self::Owned(value)
    }
}
impl From<std::sync::Arc<[u8]>> for Chunk {
    fn from(value: std::sync::Arc<[u8]>) -> Self {
        Self::Shared(value)
    }
}
impl Chunk {
    pub(crate) fn bytes(&self) -> &[u8] {
        match self {
            Self::Owned(bytes) => bytes,
            Self::Shared(bytes) => bytes,
        }
    }
    fn capacity(&self) -> usize {
        match self {
            Self::Owned(bytes) => bytes.capacity(),
            Self::Shared(bytes) => bytes.len(),
        }
    }
    #[cfg(test)]
    fn as_ptr(&self) -> *const u8 {
        self.bytes().as_ptr()
    }
}
impl Output {
    pub(crate) fn http(head: String, body: Chunk) -> Self {
        let capacity = head.capacity() + body.capacity();
        Self { chunks: vec![head.into_bytes().into(), body], capacity, limit: usize::MAX, hislip: None }
    }
    pub(crate) fn rpc(bytes: Vec<u8>) -> Self {
        let header = ((bytes.len() as u32) | 1 << 31).to_be_bytes().to_vec();
        let capacity = header.capacity() + bytes.capacity();
        Self {
            chunks: vec![header.into(), bytes.into()],
            capacity,
            limit: crate::netport::rpc::REPLY_BYTES,
            hislip: None,
        }
    }
    pub(crate) fn bytes(bytes: Vec<u8>) -> Self {
        let capacity = bytes.capacity();
        Self { chunks: vec![bytes.into()], capacity, limit: RESPONSE_BYTES, hislip: None }
    }
    pub(crate) fn hislip(bytes: Vec<u8>, msgid: u32, max: usize) -> Self {
        Self { hislip: Some((msgid, max)), ..Self::bytes(bytes) }
    }

    pub(crate) fn hislip_framing(&self) -> Option<(u32, usize)> {
        self.hislip
    }
    pub fn bounded(limit: usize) -> Self {
        Self { chunks: Vec::new(), capacity: 0, limit, hislip: None }
    }
    pub(crate) fn set_limit(&mut self, limit: usize) {
        self.limit = limit;
    }
    pub fn remaining(&self) -> usize {
        self.limit.saturating_sub(self.capacity + self.chunks.capacity() * std::mem::size_of::<Chunk>() + 64)
    }
    pub fn push(&mut self, bytes: Vec<u8>) -> Result<(), Vec<u8>> {
        if bytes.capacity() > self.remaining() {
            return Err(bytes);
        }
        self.chunks.reserve_exact(1);
        self.capacity += bytes.capacity();
        self.chunks.push(bytes.into());
        Ok(())
    }
    pub fn chunks(&self) -> impl Iterator<Item = &[u8]> {
        self.chunks.iter().map(Chunk::bytes)
    }
    pub fn capacity(&self) -> usize {
        self.capacity + self.chunks.capacity() * std::mem::size_of::<Chunk>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn allocated_capacity_including_chunk_storage_is_bounded_without_copying() {
        let mut output = Output::default();
        let bytes = Vec::with_capacity(RESPONSE_BYTES - 128);
        let allocation = bytes.as_ptr();
        assert!(output.push(bytes).is_ok());
        assert_eq!(output.chunks[0].as_ptr(), allocation);
        assert!(output.capacity() <= RESPONSE_BYTES);
        assert!(output.push(Vec::with_capacity(256)).is_err());
        assert_eq!(output.chunks.len(), 1);
    }

    #[test]
    fn output_cannot_outgrow_the_remaining_global_reservation() {
        let mut output = Output::bounded(1024);
        assert!(output.push(Vec::with_capacity(1025)).is_err());
        assert!(output.push(vec![0; 512]).is_ok());
        assert!(output.capacity() <= 1024);
        assert!(output.push(vec![0; 512]).is_err());
    }
}
