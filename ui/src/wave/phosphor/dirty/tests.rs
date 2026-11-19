// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::wave::Phosphor;

#[test]
fn consumers_keep_independent_bands_across_skipped_frames_and_replacements() {
    let mut dirty = Dirty::new(16);
    let (panel, all) = dirty.since(0, None);
    let (monitor, _) = dirty.since(0, None);
    assert_eq!(all, 0..16);
    assert_eq!(dirty.since(0, Some(&panel)).1, 0..0);
    dirty.mark(0, 2..4);
    let (panel, band) = dirty.since(0, Some(&panel));
    assert_eq!(band, 2..4);
    dirty.mark(0, 12..14);
    assert_eq!(dirty.since(0, Some(&panel)).1, 12..14);
    assert_eq!(dirty.since(0, Some(&monitor)).1, 2..14);
    assert_eq!(dirty.since(1, Some(&panel)).1, 0..16, "channel reordering invalidates the block");
    assert_eq!(Dirty::new(16).since(0, Some(&panel)).1, 0..16);
    dirty.revisions.get_mut().unwrap().revision = u64::MAX;
    dirty.mark(0, 8..9);
    assert_eq!(dirty.since(0, Some(&panel)).1, 0..16, "revision rollover invalidates old stamps");
}

fn take(ph: &Phosphor, previous: &mut Option<Stamp>, channel: usize) -> Range<usize> {
    let (stamp, band) = ph.upload_rows(channel, previous.as_ref());
    *previous = Some(stamp);
    band
}

#[test]
fn clear_decay_halve_and_merge_include_erased_and_new_rows() {
    let mut ph = Phosphor::new(4, 16);
    let mut stamp = None;
    assert_eq!(take(&ph, &mut stamp, 0), 0..16);
    let mut source = Phosphor::new(4, 16);
    source.map[0][2 * 4] = 24;
    ph.add_maps(&source, [true, false, false, false]);
    assert_eq!(take(&ph, &mut stamp, 0), 2..3);
    ph.decay(1.0);
    assert_eq!(take(&ph, &mut stamp, 0), 0..0);
    ph.decay(0.5);
    assert_eq!(take(&ph, &mut stamp, 0), 2..3);
    assert_eq!(ph.map[0][8], 12);
    ph.halve();
    assert_eq!(take(&ph, &mut stamp, 0), 2..3);
    assert_eq!(ph.map[0][8], 6);
    ph.clear();
    source.clear();
    source.map[0][12 * 4] = 24;
    ph.add_maps(&source, [true, false, false, false]);
    assert_eq!(take(&ph, &mut stamp, 0), 2..13, "cleared trace and new trace form one hull");
    assert_eq!(ph.map[0][8], 0);
    ph.clear();
    take(&ph, &mut stamp, 0);
    ph.clear();
    assert_eq!(take(&ph, &mut stamp, 0), 0..0);
}

#[test]
fn drawn_rows_cover_every_changed_pixel_in_dense_and_sparse_routes() {
    use crate::{board::Frame, model::*, sparse::Interp};
    let mut model = Model::default();
    for ch in &mut model.ch {
        ch.on = true;
    }
    for x_per_sample in [0.25, 4.0] {
        let dt = x_per_sample * model.time_per_div() as f64 * HDIV as f64 / 64.0;
        for dots in [DotsMode::Off, DotsMode::On] {
            model.dots = dots;
            let mut ph = Phosphor::new(64, 32);
            let mut stamps: [Option<Stamp>; 4] = std::array::from_fn(|_| None);
            for (c, stamp) in stamps.iter_mut().enumerate() {
                take(&ph, stamp, c);
            }
            let frame = Frame { dt, pre: 512, ch: std::array::from_fn(|_| vec![0.0; 1024]), ..Frame::default() };
            crate::drawpipe::draw_frame(&mut ph, &model, &frame, Interp::Exact, None);
            assert!(!ph.is_wide(), "this fixture must exercise ordinary narrow storage");
            for (c, stamp) in stamps.iter_mut().enumerate() {
                let band = take(&ph, stamp, c);
                assert!(!band.is_empty());
                assert!(band.end - band.start < ph.h, "a flat trace has a narrow band");
                for (y, row) in ph.map[c].chunks(ph.w).enumerate() {
                    assert!(row.iter().all(|&v| v == 0) || band.contains(&y));
                }
            }
        }
    }
}

#[test]
fn dense_spans_publish_visible_rows_and_leave_empty_revisions_unchanged() {
    use crate::{board::Frame, model::*, sparse::Interp};
    let mut model = Model { dots: DotsMode::Off, ..Model::default() };
    for ch in &mut model.ch {
        ch.on = true;
        ch.offset = 0.0;
    }
    for (level, delay) in [(0.0, 0.0), (-100.0, 0.0), (100.0, 0.0), (f32::NAN, 0.0), (0.0, 1.0)] {
        model.delay = delay;
        let frame = Frame { dt: 1e-9, pre: 512, ch: std::array::from_fn(|_| vec![level; 1024]), ..Frame::default() };
        let mut ph = Phosphor::new(64, 32);
        let stamps: [_; NCH] = std::array::from_fn(|c| ph.upload_rows(c, None).0);
        crate::drawpipe::draw_frame(&mut ph, &model, &frame, Interp::Exact, None);
        for (c, stamp) in stamps.iter().enumerate() {
            let visible = level == 0.0 && delay == 0.0;
            let (next, rows) = ph.upload_rows(c, Some(stamp));
            assert_eq!(rows, if visible { 16..17 } else { 0..0 });
            assert_eq!(next.revision == stamp.revision, !visible);
            assert_eq!(ph.maps()[c].iter().any(|&v| v != 0), visible);
            for (y, row) in ph.maps()[c].chunks(ph.w).enumerate() {
                assert!(row.iter().all(|&v| v == 0) || rows.contains(&y));
            }
        }
    }
}

#[test]
fn worker_mutations_need_no_upload_history_and_first_publication_is_complete() {
    let mut dirty = Dirty::new(16);
    for _ in 0..100 {
        dirty.mark(0, 2..4);
    }
    assert!(dirty.revisions.get().is_none());
    let (first, band) = dirty.since(0, None);
    assert_eq!(band, 0..16);
    dirty.mark(0, 8..9);
    assert_eq!(dirty.since(0, Some(&first)).1, 8..9);
}

#[test]
fn channel_mutations_record_rows_without_a_separate_mark() {
    let mut ph = Phosphor::new(4, 16);
    let mut stamps: [Option<Stamp>; NCH] = std::array::from_fn(|_| None);
    for (c, stamp) in stamps.iter_mut().enumerate() {
        take(&ph, stamp, c);
    }
    ph.edit_channel(1, |map| {
        map[8] = 24;
        2..3
    });
    assert_eq!(take(&ph, &mut stamps[0], 0), 0..0);
    assert_eq!(take(&ph, &mut stamps[1], 1), 2..3);
    ph.clear_channel(1);
    assert_eq!(take(&ph, &mut stamps[1], 1), 0..16);
    assert!(ph.maps()[1].iter().all(|&value| value == 0));
    assert_eq!(take(&ph, &mut stamps[0], 0), 0..0);
}

#[test]
fn merging_saturated_or_unused_channels_does_not_dirty_them() {
    let mut ph = Phosphor::new(4, 16);
    let mut source = Phosphor::new(4, 16);
    ph.channel_mut(0)[8] = u16::MAX;
    source.channel_mut(0)[8] = 24;
    source.channel_mut(1)[8] = 24;
    let mut stamp = None;
    take(&ph, &mut stamp, 0);
    ph.add_maps(&source, [true, false, false, false]);
    assert_eq!(take(&ph, &mut stamp, 0), 0..0);
    assert_eq!(ph.maps()[0][8], u16::MAX);
    assert!(ph.maps()[1].iter().all(|&value| value == 0));
}

#[test]
fn row_marks_clamp_to_the_map_height_without_advancing_empty_revisions() {
    for height in [0, 16] {
        let mut dirty = Dirty::new(height);
        let (stamp, _) = dirty.since(0, None);
        for rows in [height..height + 4, height + 2..height + 8, Range { start: 8, end: 4 }] {
            dirty.mark(0, rows);
            let (next, band) = dirty.since(0, Some(&stamp));
            assert_eq!(band, 0..0);
            assert_eq!(next.revision, stamp.revision);
        }
        dirty.mark(0, 2..height + 4);
        assert_eq!(dirty.since(0, Some(&stamp)).1, if height == 0 { 0..0 } else { 2..height });
        assert_eq!(dirty.publish(), if height == 0 { 0..0 } else { 2..height });
        assert_eq!(dirty.publish(), 0..0);
    }
}

#[test]
fn folding_rows_beyond_the_map_height_tracks_only_the_clipped_rows() {
    let mut ph = Phosphor::new(4, 16);
    let mut stamp = None;
    take(&ph, &mut stamp, 0);
    let mut accumulators = std::array::from_fn(|_| vec![0; 4 * 17]);
    accumulators[0][12 * 4..13 * 4].fill(24);
    ph.fold_rows(&mut accumulators, &[0], 4, [(12, 20); NCH]);
    assert_eq!(take(&ph, &mut stamp, 0), 12..16);
    assert!(ph.maps()[0][..12 * 4].iter().all(|&value| value == 0));
    assert!(ph.maps()[0][12 * 4..].iter().all(|&value| value == 24));
    ph.fold_rows(&mut accumulators, &[0], 4, [(20, 24); NCH]);
    assert_eq!(take(&ph, &mut stamp, 0), 0..0);
}

#[test]
fn unary_transforms_leave_zero_and_single_hit_rows_clean() {
    let mut ph = Phosphor::new(4, 16);
    ph.map[0][2 * 4] = 1;
    ph.map[0][12 * 4] = 2;
    let mut stamp = None;
    take(&ph, &mut stamp, 0);
    ph.halve();
    assert_eq!(take(&ph, &mut stamp, 0), 12..13);
    assert_eq!(ph.map[0][2 * 4], 1);
    assert_eq!(ph.map[0][12 * 4], 1);
    ph.halve();
    assert_eq!(take(&ph, &mut stamp, 0), 0..0);
    ph.decay(0.5);
    assert_eq!(take(&ph, &mut stamp, 0), 2..13);
    assert!(ph.map[0].iter().all(|&value| value == 0));
    ph.decay(0.0);
    assert_eq!(take(&ph, &mut stamp, 0), 0..0);
}

#[test]
fn standard_draws_publish_only_the_active_channels_exact_rows() {
    use crate::{board::Frame, model::*, sparse::Interp};
    let mut model = Model::default();
    for (c, ch) in model.ch.iter_mut().enumerate() {
        ch.on = c == 1;
        ch.offset = 0.0;
    }
    for dots in [DotsMode::Off, DotsMode::On] {
        model.dots = dots;
        let dt = 0.25 * model.time_per_div() as f64 * HDIV as f64 / 64.0;
        let frame = Frame { dt, pre: 512, ch: std::array::from_fn(|_| vec![0.0; 1024]), ..Frame::default() };
        let mut ph = Phosphor::new(64, 32);
        let stamps: [_; NCH] = std::array::from_fn(|c| ph.upload_rows(c, None).0);
        crate::drawpipe::draw_frame(&mut ph, &model, &frame, Interp::Exact, None);
        assert!(!ph.is_wide(), "exercise true narrow Dots as well as Lines");
        ph.publish();
        assert_eq!(ph.published_rows(), 16..17);
        for (c, stamp) in stamps.iter().enumerate() {
            assert_eq!(ph.upload_rows(c, Some(stamp)).1, if c == 1 { 16..17 } else { 0..0 });
        }
        ph.publish();
        assert_eq!(ph.published_rows(), 0..0);
        assert_eq!(ph.upload_rows(1, Some(&stamps[1])).1, 16..17);
    }
}

#[test]
fn count_transforms_publish_the_same_exact_channel_changes_in_both_formats() {
    use crate::picture::deposit::count::CountFormat;
    for format in [CountFormat::Narrow, CountFormat::Wide] {
        let mut ph = Phosphor::new(4, 16);
        ph.set_format(format);
        for (c, row, value) in [(0, 2, 24), (1, 12, 1)] {
            if ph.is_wide() {
                ph.edit_wide_channel(c, |map| {
                    map[row * 4] = value;
                    row..row + 1
                });
            } else {
                ph.edit_channel(c, |map| {
                    map[row * 4] = value as u16;
                    row..row + 1
                });
            }
        }
        ph.publish();
        let mut stamps: [_; NCH] = std::array::from_fn(|c| Some(ph.upload_rows(c, None).0));
        ph.halve();
        ph.publish();
        assert_eq!(ph.published_rows(), 2..3);
        for (c, stamp) in stamps.iter_mut().enumerate() {
            assert_eq!(take(&ph, stamp, c), if c == 0 { 2..3 } else { 0..0 });
        }
        ph.decay(0.5);
        ph.publish();
        assert_eq!(ph.published_rows(), 2..13);
        for (c, stamp) in stamps.iter_mut().enumerate() {
            assert_eq!(
                take(&ph, stamp, c),
                match c {
                    0 => 2..3,
                    1 => 12..13,
                    _ => 0..0,
                }
            );
        }
        ph.clear_counts();
        ph.publish();
        assert_eq!(ph.published_rows(), 2..3);
        assert_eq!(take(&ph, &mut stamps[0], 0), 2..3);
        ph.clear_counts();
        ph.halve();
        ph.decay(0.0);
        ph.publish();
        assert_eq!(ph.published_rows(), 0..0);
        for (c, stamp) in stamps.iter_mut().enumerate() {
            assert_eq!(take(&ph, stamp, c), 0..0);
        }
    }
}

#[test]
fn format_changes_invalidate_uploads_without_publishing_unchanged_counts() {
    use crate::picture::deposit::count::CountFormat;
    let mut ph = Phosphor::new(4, 16);
    ph.edit_channel(1, |map| {
        map[48] = 24;
        12..13
    });
    ph.publish();
    let mut stamps: [_; NCH] = std::array::from_fn(|c| Some(ph.upload_rows(c, None).0));
    for format in [CountFormat::Wide, CountFormat::Wide, CountFormat::Narrow, CountFormat::Narrow] {
        let changed = ph.is_wide() != (format == CountFormat::Wide);
        let cleared = changed && format == CountFormat::Narrow;
        ph.set_format(format);
        ph.publish();
        assert_eq!(ph.published_rows(), if cleared { 12..13 } else { 0..0 });
        assert_eq!(ph.count(1, 48), if ph.is_wide() { 24 } else { 0 });
        for (c, stamp) in stamps.iter_mut().enumerate() {
            assert_eq!(take(&ph, stamp, c), if changed { 0..16 } else { 0..0 });
        }
    }
}
