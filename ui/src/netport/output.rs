// SPDX-License-Identifier: GPL-2.0-or-later
//! Response chunks move from interpreter to IO without a second assembled copy.
use super::RESPONSE_BYTES;

pub struct Output {
    chunks: Vec<Vec<u8>>,
    capacity: usize,
    limit: usize,
}
impl Default for Output {
    fn default() -> Self {
        Self::bounded(RESPONSE_BYTES)
    }
}
impl Output {
    pub fn bounded(limit: usize) -> Self {
        Self { chunks: Vec::new(), capacity: 0, limit }
    }
    pub fn remaining(&self) -> usize {
        self.limit.saturating_sub(self.capacity + self.chunks.capacity() * std::mem::size_of::<Vec<u8>>() + 64)
    }
    pub fn push(&mut self, bytes: Vec<u8>) -> Result<(), Vec<u8>> {
        if bytes.capacity() > self.remaining() {
            return Err(bytes);
        }
        self.chunks.reserve_exact(1);
        self.capacity += bytes.capacity();
        self.chunks.push(bytes);
        Ok(())
    }
    pub fn chunks(&self) -> impl Iterator<Item = &[u8]> {
        self.chunks.iter().map(Vec::as_slice)
    }
    pub fn capacity(&self) -> usize {
        self.capacity + self.chunks.capacity() * std::mem::size_of::<Vec<u8>>()
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
