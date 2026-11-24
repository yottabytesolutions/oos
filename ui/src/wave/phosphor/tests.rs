// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::picture::deposit::count::CountFormat;

#[test]
fn publication_keeps_consumer_history_and_channel_edits_independent() {
    let mut ph = Phosphor::new(4, 16);
    let (first, _) = ph.upload_rows(0, None);
    let (other, _) = ph.upload_rows(1, None);
    ph.edit_channel(0, |map| {
        map[8] = 24;
        2..3
    });
    ph.publish();
    assert_eq!(ph.published_rows(), 2..3);
    ph.publish();
    assert!(ph.published_rows().is_empty());
    assert_eq!(ph.upload_rows(0, Some(&first)).1, 2..3);
    assert_eq!(ph.upload_rows(1, Some(&other)).1, 0..0);
    ph.clear_counts();
    ph.publish();
    assert_eq!(ph.published_rows(), 2..3);
    ph.clear_counts();
    ph.publish();
    assert!(ph.published_rows().is_empty());
}

#[test]
fn folded_picture_rows_survive_publication_until_each_consumer_uploads() {
    let mut ph = Phosphor::new(4, 16);
    let (first, _) = ph.upload_rows(0, None);
    ph.edit_channel(0, |map| {
        map[8] = 24;
        2..3
    });
    ph.publish();
    assert_eq!(ph.published_rows(), 2..3);
    assert_eq!(ph.upload_rows(0, Some(&first)).1, 2..3);
    let (panel, _) = ph.upload_rows(0, Some(&first));
    ph.edit_channel(0, |map| {
        map[48] = 24;
        12..13
    });
    ph.publish();
    assert_eq!(ph.published_rows(), 12..13);
    assert_eq!(ph.upload_rows(0, Some(&panel)).1, 12..13);
    assert_eq!(ph.upload_rows(0, Some(&first)).1, 2..13);
}

#[test]
fn clearing_wide_storage_invalidates_previous_narrow_upload_rows() {
    let mut ph = Phosphor::new(4, 16);
    ph.edit_channel(0, |map| {
        map[8] = 24;
        2..3
    });
    ph.publish();
    let (narrow, _) = ph.upload_rows(0, None);
    ph.set_format(CountFormat::Wide);
    ph.edit_wide_channel(0, |map| {
        map[48] = 70_000;
        12..13
    });
    ph.clear();
    ph.publish();
    assert_eq!(ph.published_rows(), 2..13);
    assert!(!ph.is_wide());
    assert!(ph.maps().iter().flatten().all(|&value| value == 0));
    assert_eq!(ph.upload_rows(0, Some(&narrow)).1, 0..16);
}

#[test]
fn fully_decayed_traces_stop_publishing_rows() {
    for format in [CountFormat::Narrow, CountFormat::Wide] {
        let mut ph = Phosphor::new(2, 4);
        ph.edit_channel(NCH - 1, |map| {
            map[4] = 2;
            2..3
        });
        ph.set_format(format);
        ph.publish();
        assert_eq!(ph.published_rows(), 2..3);
        ph.decay(1.0);
        ph.publish();
        assert!(ph.published_rows().is_empty());
        for expected in [1, 0] {
            ph.decay(0.5);
            ph.publish();
            assert_eq!(ph.count(NCH - 1, 4), expected);
            assert_eq!(ph.published_rows(), 2..3);
        }
        for factor in [0.5, 0.0, 0.5] {
            ph.decay(factor);
            ph.publish();
            assert!(ph.published_rows().is_empty());
        }
    }
}

#[test]
fn channel_maximum_follows_edits_decay_and_format_changes() {
    let mut ph = Phosphor::new(2, 4);
    ph.edit_channel(0, |map| {
        map[4] = 24;
        2..3
    });
    assert_eq!(ph.channel_max(0), 24);
    assert_eq!(ph.merged_max([true, false, false, false]), 24);
    ph.decay(0.5);
    assert_eq!(ph.channel_max(0), 12);
    ph.set_format(CountFormat::Wide);
    assert_eq!(ph.channel_max(0), 12);
    ph.edit_wide_channel(0, |map| {
        map[4] = 70_000;
        2..3
    });
    assert_eq!(ph.channel_max(0), 70_000);
    ph.clear();
    assert_eq!(ph.channel_max(0), 0);
}
