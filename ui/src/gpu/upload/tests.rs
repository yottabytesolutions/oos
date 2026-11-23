// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::wave::Phosphor;

#[test]
fn empty_pixel_sources_need_no_rows_on_first_or_repeated_publication() {
    let mut dst = [];
    let pixels = || UploadData::Pixels { bytes: &[], row_bytes: 0 };
    let (block, rows) = Block::publish(0, &mut dst, pixels(), None);
    assert_eq!(rows, 0..0);
    assert_eq!(Block::publish(0, &mut dst, pixels(), Some(&block)).1, 0..0);
    assert_eq!(copy_rows(&mut dst, &[], 0, None), 0..0);
    assert_eq!(copy_rows(&mut dst, &[], 0, Some(0..0)), 0..0);
}

#[test]
fn dirty_copy_touches_only_the_band_and_pixel_fallback_finds_the_same_hull() {
    let src = vec![7; 32];
    let mut dst = vec![99; 32];
    assert_eq!(copy_rows(&mut dst, &src, 4, Some(2..4)), 2..4);
    assert_eq!(&dst[..8], &[99; 8]);
    assert_eq!(&dst[8..16], &[7; 8]);
    assert_eq!(&dst[16..], &[99; 16]);
    let before = dst.clone();
    assert_eq!(copy_rows(&mut dst, &src, 4, Some(0..0)), 0..0);
    assert_eq!(before, dst);
    assert_eq!(copy_rows(&mut dst, &src, 4, None), 0..8);
    assert_eq!(src, dst);
    assert_eq!(copy_rows(&mut dst, &src, 4, None), 0..0);
}

#[test]
fn pixel_blocks_initialize_then_compare_rows() {
    let mut src = vec![7; 32];
    let mut dst = vec![99; 32];
    fn pixels(bytes: &[u8]) -> UploadData<'_> {
        UploadData::Pixels { bytes, row_bytes: 4 }
    }
    let (block, band) = Block::publish(64, &mut dst, pixels(&src), None);
    assert_eq!(band, 0..8);
    assert_eq!(dst, src);
    assert_eq!(Block::publish(64, &mut dst, pixels(&src), Some(&block)).1, 0..0);
    src[8] = 24;
    src[20] = 24;
    assert_eq!(Block::publish(64, &mut dst, pixels(&src), Some(&block)).1, 2..6);
    assert_eq!(dst, src);
}

fn publish(ph: &Phosphor, previous: &mut Option<Block>, arena: &mut [u8]) -> Range<usize> {
    let (block, band) = Block::publish(0, arena, UploadData::Channel { phosphor: ph, channel: 0 }, previous.as_ref());
    *previous = Some(block);
    band
}

#[test]
fn shared_buffers_match_the_picture_after_clear_replacement_and_skipped_monitor_frames() {
    let mut ph = Phosphor::new(4, 16);
    let mut stamps = [None, None];
    let mut arenas = [vec![99; 128], vec![99; 128]];
    for (stamp, arena) in stamps.iter_mut().zip(&mut arenas) {
        publish(&ph, stamp, arena);
    }
    for step in 0..16 {
        if step % 3 == 0 {
            ph.clear();
        }
        let mut source = Phosphor::new(4, 16);
        source.channel_mut(0)[step * 4] = 24;
        ph.add_maps(&source, [true, false, false, false]);
        publish(&ph, &mut stamps[0], &mut arenas[0]);
        if step % 2 == 0 {
            publish(&ph, &mut stamps[1], &mut arenas[1]);
        }
        let bytes: Vec<_> = ph.maps()[0].iter().flat_map(|value| value.to_ne_bytes()).collect();
        assert_eq!(arenas[0], bytes);
        if step % 2 == 0 {
            assert_eq!(arenas[1], bytes);
        }
    }
    let replacement = Phosphor::new(4, 16);
    assert_eq!(publish(&replacement, &mut stamps[0], &mut arenas[0]), 0..16);
    assert!(arenas[0].iter().all(|&v| v == 0));
}

thread_local! {
    static WORK: std::cell::Cell<Option<[usize; 3]>> = const { std::cell::Cell::new(None) };
}

pub(super) fn record_work(compared: usize, copied: usize, copies: usize) {
    if let Some([a, b, c]) = WORK.get() {
        WORK.set(Some([a + compared, b + copied, c + copies]));
    }
}

#[test]
#[ignore = "host upload work: OOS_PERF_TESTS=1"]
fn perf_guard_upload_rows() {
    upload_work(200..208);
}

#[test]
#[ignore = "host upload work: OOS_PERF_TESTS=1"]
fn perf_guard_upload_full() {
    upload_work(0..448);
}

fn upload_work(band: Range<usize>) {
    assert_eq!(std::env::var("OOS_PERF_TESTS").as_deref(), Ok("1"));
    let (w, h) = (970, 448);
    let mut ph = Phosphor::new(w, h);
    for c in 0..4 {
        let mut dst = vec![0; w * h * 2];
        let (old, _) = Block::publish(0, &mut dst, UploadData::Channel { phosphor: &ph, channel: c }, None);
        ph.edit_channel(c, |map| {
            map[band.start * w..band.end * w].fill(24);
            band.clone()
        });
        WORK.set(Some([0; 3]));
        let (_, copied) = Block::publish(0, &mut dst, UploadData::Channel { phosphor: &ph, channel: c }, Some(&old));
        let work = WORK.take().unwrap();
        assert_eq!(copied, band);
        assert_eq!(work, [0, band.len(), 1], "no scan and one contiguous copy per channel");
        assert_eq!(dst, (UploadData::Channel { phosphor: &ph, channel: c }).bytes());
    }
    println!("UPLOAD_WORK channels=4 compared_rows=0 copied_rows={} copies=4", 4 * band.len());
}

#[test]
fn new_moved_resized_and_reordered_blocks_initialize_every_row() {
    let mut ph = Phosphor::new(4, 16);
    ph.channel_mut(0).fill(24);
    let mut dst = vec![99; 128];
    let (block, band) = Block::publish(64, &mut dst, UploadData::Channel { phosphor: &ph, channel: 0 }, None);
    assert_eq!(band, 0..16);
    let expected: Vec<_> = ph.maps()[0].iter().flat_map(|value| value.to_ne_bytes()).collect();
    assert_eq!(dst, expected);
    assert_eq!(Block::publish(64, &mut dst, UploadData::Channel { phosphor: &ph, channel: 0 }, Some(&block)).1, 0..0);
    assert_eq!(
        Block::publish(
            128,
            &mut dst,
            UploadData::Channel { phosphor: &ph, channel: 0 },
            Some(&block).filter(|block| block.matches(128, 128))
        )
        .1,
        0..16
    );
    assert_eq!(Block::publish(64, &mut dst, UploadData::Channel { phosphor: &ph, channel: 1 }, Some(&block)).1, 0..16);
    assert!(dst.iter().all(|&byte| byte == 0), "the selected channel supplies the bytes");
    let other = Phosphor::new(4, 16);
    assert_eq!(
        Block::publish(64, &mut dst, UploadData::Channel { phosphor: &other, channel: 0 }, Some(&block)).1,
        0..16
    );
    let smaller = Phosphor::new(4, 8);
    assert_eq!(
        Block::publish(64, &mut dst[..64], UploadData::Channel { phosphor: &smaller, channel: 0 }, Some(&block)).1,
        0..8
    );
}

#[test]
fn count_sources_use_narrow_revisions_and_preserve_wide_indices() {
    let mut ph = Phosphor::new(2, 8);
    ph.edit_channel(0, |map| {
        map[4] = 24;
        2..3
    });
    let mut arena = vec![0; 32];
    let (block, _) = Block::publish(0, &mut arena, UploadData::counts(&ph, 0, &[]), None);
    assert!(block.stamp.is_some());
    ph.set_format(crate::picture::deposit::count::CountFormat::Wide);
    ph.wide_map_mut(0)[12] = 70_000;
    let mut encoded = vec![0; 16];
    encoded[12] = 2047;
    let (block, rows) = Block::publish(0, &mut arena, UploadData::counts(&ph, 0, &encoded), Some(&block));
    assert!(block.stamp.is_none());
    assert_eq!(rows, 2..7);
    assert_eq!(arena, super::super::bytes(encoded.as_slice()));
    assert_eq!(Block::publish(0, &mut arena, UploadData::counts(&ph, 0, &encoded), Some(&block)).1, 0..0);
    ph.clear();
    assert_eq!(Block::publish(0, &mut arena, UploadData::counts(&ph, 0, &[]), Some(&block)).1, 0..8);
    assert!(arena.iter().all(|&v| v == 0));
}

#[test]
fn a_monitor_skipping_wide_pictures_clears_its_old_narrow_rows() {
    use crate::picture::deposit::count::CountFormat;
    let mut ph = Phosphor::new(2, 16);
    ph.edit_channel(0, |map| {
        map[4] = 24;
        2..3
    });
    let mut arena = vec![0; 64];
    let (block, _) = Block::publish(0, &mut arena, UploadData::counts(&ph, 0, &[]), None);
    ph.set_format(CountFormat::Wide);
    ph.clear_counts();
    ph.edit_wide_channel(0, |map| {
        map[24] = 70_000;
        12..13
    });
    ph.clear_counts();
    ph.set_format(CountFormat::Narrow);
    ph.edit_channel(0, |map| {
        map[16] = 24;
        8..9
    });
    let (_, rows) = Block::publish(0, &mut arena, UploadData::counts(&ph, 0, &[]), Some(&block));
    assert_eq!(rows, 0..16, "storage changes invalidate a skipped consumer's old narrow stamp");
    assert_eq!(arena, super::super::bytes(ph.maps()[0].as_slice()));
}
