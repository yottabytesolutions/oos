// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;
use crate::picture::deposit::{count::CountFormat, HIT_LIMIT};

fn deposit(format: CountFormat) -> Deposit {
    let mut d = Deposit::new(7, (17, 9), None, [true; 4]);
    d.set_format(format);
    d.records = 3;
    d.triggered = 2;
    d.edges = [1, 2, 3, 4];
    d.extent.add_hits(3);
    for b in &mut d.planes {
        b.test_span(8, 24);
    }
    d
}

#[test]
fn same_picture_fold_matches_window_oracle_without_metadata_or_allocation() {
    for format in [CountFormat::Narrow, CountFormat::Wide] {
        let mut a = deposit(format);
        let b = deposit(format);
        let mut oracle = deposit(format);
        let mut expected = crate::wave::Phosphor::new(17, 9);
        let mut actual = crate::wave::Phosphor::new(17, 9);
        expected.set_format(format);
        actual.set_format(format);
        for (out, source) in oracle.planes.iter_mut().zip(&b.planes) {
            out.fold_picture([Some(source), None], &mut expected);
        }
        let (allocations, totals) = crate::system::test_allocator::count_own(|| a.merge_picture_planes(&[&b]));
        assert_eq!(allocations, 0);
        assert_eq!(totals, Some(PictureTotals { records: 6, triggered: 4, hits: 6, edges: [2, 4, 6, 8] }));
        assert_eq!((a.records, a.triggered, a.edges, a.extent.hits()), (3, 2, [1, 2, 3, 4], 3));
        assert_eq!((b.records, b.triggered, b.edges, b.extent.hits()), (3, 2, [1, 2, 3, 4], 3));
        for band in &mut a.planes {
            band.fold_picture([None, None], &mut actual);
        }
        for c in 0..4 {
            for pixel in 0..17 * 9 {
                assert_eq!(actual.count(c, pixel), expected.count(c, pixel));
            }
        }
    }
}

#[test]
fn whole_selection_rejects_late_mismatch_and_all_integer_overflows_before_mutation() {
    let mut a = deposit(CountFormat::Narrow);
    let b = deposit(CountFormat::Narrow);
    let mut bad = deposit(CountFormat::Narrow);
    bad.planes[3].channel = 0;
    assert!(a.merge_picture_planes(&[&b, &bad]).is_none());
    // The subsequent valid merge must still have exactly two contributions.
    let mut ph = crate::wave::Phosphor::new(17, 9);
    assert!(a.merge_picture_planes(&[&b]).is_some());
    for band in &mut a.planes {
        band.fold_picture([None, None], &mut ph);
    }
    assert_eq!(ph.count(0, 8 * 17), 48);
    for field in 0..4 {
        let mut a = deposit(CountFormat::Narrow);
        match field {
            0 => a.records = u64::MAX,
            1 => a.triggered = u64::MAX,
            2 => a.edges[2] = u64::MAX,
            _ => a.extent.add_hits(u64::MAX),
        }
        assert!(a.merge_picture_planes(&[&b]).is_none());
    }
    let mut a = deposit(CountFormat::Narrow);
    a.extent = Default::default();
    a.extent.add_hits(HIT_LIMIT - 3);
    assert!(a.picture_totals(&[&b]).is_some());
    a.extent.add_hits(1);
    assert!(a.merge_picture_planes(&[&b]).is_none());
}

#[test]
fn format_identity_geometry_and_five_input_limit_are_preflighted() {
    let a = deposit(CountFormat::Narrow);
    let mut b = deposit(CountFormat::Wide);
    assert!(a.picture_totals(&[&b]).is_none());
    b = deposit(CountFormat::Narrow);
    b.maps.gen += 1;
    assert!(a.picture_totals(&[&b]).is_none());
    b = deposit(CountFormat::Narrow);
    let (w, h) = b.size();
    b.maps.sizes = crate::picture::PerMap::new((w + 1, h), None);
    assert!(a.picture_totals(&[&b]).is_none());
    b = deposit(CountFormat::Narrow);
    b.identity = Some(Default::default());
    assert!(a.picture_totals(&[&b]).is_none());
    b = deposit(CountFormat::Narrow);
    b.off.0 = 1.0;
    assert!(a.picture_totals(&[&b]).is_some(), "off statistics are independently credited, not merge geometry");
    b = deposit(CountFormat::Narrow);
    assert!(a.picture_totals(&[&b; 4]).is_some());
    assert!(a.picture_totals(&[&b; 5]).is_none());
}

#[test]
fn statistics_generation_is_part_of_group_identity_even_when_pair_matches() {
    use oos_core::change::{Request, Shadow, UiAcq};
    let model = crate::model::Model::default();
    let config = crate::model::config();
    let mut shadow = Shadow::new(&model, UiAcq::default(), &config).unwrap();
    let before = shadow.identity();
    let after = shadow.observe(&model, UiAcq::default(), &[Request::StatsReset], &config).unwrap().identity();
    assert_eq!((before.gen(), before.epoch()), (after.gen(), after.epoch()));
    assert_ne!(before.stats_gen(), after.stats_gen());
    let mut a = deposit(CountFormat::Narrow);
    let mut b = deposit(CountFormat::Narrow);
    a.identity = Some(before);
    b.identity = Some(after);
    assert!(a.merge_picture_planes(&[&b]).is_none());
    b.identity = Some(before);
    assert!(a.merge_picture_planes(&[&b]).is_some());
}
