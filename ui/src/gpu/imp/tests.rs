// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use std::{
    fs::File,
    mem::{offset_of, size_of},
    os::unix::fs::FileExt,
};

/// Drives the real frame writer and reply reader over a local stream, with a private arena.
pub(crate) struct HelperFixture {
    socket: UnixStream,
    arena: File,
}

impl HelperFixture {
    pub(crate) fn connect(w: i32, h: i32) -> (Gpu, Self) {
        let (ui, helper) = UnixStream::pair().unwrap();
        helper.set_nonblocking(true).unwrap();
        let arena = Gpu::arena_file().unwrap();
        let mut gpu = Gpu::attach(None, ui, Gpu::map_arena(&arena).unwrap());
        gpu.targets[..3].fill(Some((w, h)));
        gpu.monitor_targets_from(3);
        (gpu, Self { socket: helper, arena: arena.into() })
    }

    pub(crate) fn frame(&mut self) -> Option<usize> {
        let mut frame = [0; size_of::<FrameMsg>()];
        self.socket.read_exact(&mut frame).unwrap();
        let field = |offset: usize| u32::from_ne_bytes(frame[offset..offset + 4].try_into().unwrap());
        assert_eq!(field(offset_of!(FrameMsg, magic)), MAGIC);
        assert_eq!(field(offset_of!(FrameMsg, kind)), K_FRAME);
        (field(offset_of!(FrameMsg, readback)) != 0).then(|| field(offset_of!(FrameMsg, readback_off)) as usize)
    }

    pub(crate) fn rendered(&mut self) {
        self.reply(K_DONE);
    }

    pub(crate) fn pixels(&mut self, offset: usize, marker: u32) {
        assert!(offset <= ARENA - 4 && offset.is_multiple_of(4));
        self.arena.write_all_at(&marker.to_ne_bytes(), offset as u64).unwrap();
        self.reply(K_READBACK);
    }

    fn reply(&mut self, kind: u32) {
        let words = [MAGIC, kind, 0, 0, 0, 11.0f32.to_bits(), 0, 0].map(u32::to_ne_bytes);
        self.socket.write_all(words.as_flattened()).unwrap();
    }
}
