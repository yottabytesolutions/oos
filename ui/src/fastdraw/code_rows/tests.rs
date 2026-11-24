// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::fastdraw::CorrectedCodes;
use oos_hal::dho::samples::volt_code;

fn codes(scale: f64, offset: f64, corrected: bool) -> Codes<'static> {
    Codes {
        bytes: &[],
        words: None,
        stride: 1,
        word: 0,
        n: 0,
        affine: ((scale / 7500.0) as f32, offset as f32),
        corrected: corrected.then_some(CorrectedCodes { offsets: [0; 8], scale, offset }),
        volt: volt_code,
    }
}

fn map(y_per_volt: f32, y0: f32) -> Map {
    Map { ti: 0.0, x_per_sample: 1.0, x0: 0.0, y_per_volt, y0 }
}

fn assert_rows(codes: &Codes<'_>, map: &Map, table: &RowTable, height: i32) {
    for code in 0..=u16::MAX {
        let volts = codes.calibrated(code, code).0;
        let expected = ((volts * map.y_per_volt + map.y0).round() as i32).clamp(-1, height);
        assert_eq!(table.row(code), expected, "{map:?}, code={code}");
        assert_eq!(table.span(code, code), mapping::mapped_rows((volts, volts), None, map, height as usize));
    }
}

#[test]
fn every_code_matches_both_decoders_in_both_directions() {
    assert_eq!(size_of::<[u32; ENTRIES]>(), 8 * 1024);
    let mut cache = RowCache::default();
    for corrected in [false, true] {
        for (scale, offset) in [(1.203, 0.13), (-0.51, -0.4), (4.25, 32768.125), (0.0, -2.5), (3e28, 2e27)] {
            let codes = codes(scale, offset, corrected);
            for (slope, origin, height) in [
                (-56.25, 239.5, 480),
                (39.1, 118.39, 240),
                (6000.0, 0.5, 480),
                (-34.0, -1_114_000.0, 480),
                (1e20, 250.0, 480),
                (0.0, -0.5, 1),
            ] {
                let map = map(slope, origin);
                let table = cache.get(&codes, &map, height).unwrap();
                assert_rows(&codes, &map, table, height as i32);
            }
        }
    }
}

#[test]
fn large_cancellation_preserves_skipped_rows() {
    let codes = codes(0.75, -33_554_432.0, true);
    let map = map(8.0, -268_435_200.0);
    let mut cache = RowCache::default();
    let table = cache.get(&codes, &map, 480).unwrap();
    assert_rows(&codes, &map, table, 480);
}

#[test]
fn lookup_preserves_offscreen_sentinels() {
    let codes = codes(1.203, 0.13, true);
    let map = map(-568.25, 239.5);
    let mut cache = RowCache::default();
    let table = cache.get(&codes, &map, 480).unwrap();
    assert_eq!((table.row(0), table.row(u16::MAX)), (480, -1));
    assert_eq!(table.span(0, u16::MAX), (0, 479));
    assert_rows(&codes, &map, table, 480);
}

#[test]
fn cache_covers_decoder_and_projection_changes() {
    let mut cache = RowCache::default();
    let codes = codes(1.0, 0.0, false);
    let mut map = map(-100.0, 200.0);
    assert_eq!(cache.get(&codes, &map, 480).unwrap().row(32768), 200);
    // Phase offsets and the horizontal map do not change the row for an already corrected code.
    map.ti = 23.0;
    assert_eq!(cache.get(&codes, &map, 480).unwrap().row(32768), 200);
    assert_eq!(cache.rebuilds(), 1);
    let identity = Codes { volt: |x, _| f32::from(x), ..codes };
    assert_eq!(cache.get(&identity, &map, 480).unwrap().row(32768), -1);
    map.y0 = 100.0;
    assert_eq!(cache.get(&codes, &map, 480).unwrap().row(32768), 100);
    assert_eq!(cache.get(&codes, &map, 80).unwrap().row(32768), 80);
    assert_eq!(cache.get(&codes, &map, 1000).unwrap().height, 1000);
    map.y_per_volt = f32::NAN;
    assert!(cache.get(&codes, &map, 480).is_none());
}

#[test]
fn alternating_pane_heights_reuse_rows_and_eviction_reuses_storage() {
    let mut cache = RowCache::default();
    let codes = codes(1.0, 0.0, false);
    let map = map(-100.0, 200.0);
    let overview = cache.get(&codes, &map, 96).unwrap().entries.as_ptr();
    let detail = cache.get(&codes, &map, 352).unwrap().entries.as_ptr();
    assert_eq!(cache.rebuilds(), 2);
    assert_ne!(overview, detail);
    for _ in 0..4 {
        assert_eq!(cache.get(&codes, &map, 96).unwrap().entries.as_ptr(), overview);
        assert_eq!(cache.get(&codes, &map, 352).unwrap().entries.as_ptr(), detail);
    }
    assert_eq!(cache.rebuilds(), 2);
    assert_eq!(cache.get(&codes, &map, 480).unwrap().entries.as_ptr(), overview);
    assert_eq!(cache.rebuilds(), 3);
}

#[test]
fn compact_lanes_match_scalar_rounding_across_bucket_boundaries() {
    let mut cache = RowCache::default();
    let mut spans = Vec::with_capacity(65536);
    for height in [448, 480, 720, 1500, 32768] {
        for corrected in [false, true] {
            for (scale, offset, slope, origin) in
                [(7500.0, 0.0, 1.0, 0.0), (-7500.0, 0.125, 0.02, 0.5), (75.0, 33_554_432.0, 0.5, 16_777_200.0)]
            {
                let codes = codes(scale, offset, corrected);
                let map = map(slope, origin);
                let Some(table) = cache.get(&codes, &map, height) else {
                    assert_eq!(height, 32768);
                    continue;
                };
                let pairs = (0..=u16::MAX).map(|code| (code, code.saturating_add(code.wrapping_mul(241) & 127)));
                spans.clear();
                table.write_spans(pairs.clone(), &mut spans);
                assert_eq!(spans.len(), 65536);
                for ((low, high), &span) in pairs.zip(&spans) {
                    let extrema = codes.calibrated(low, high);
                    assert_eq!(span, mapping::mapped_rows(extrema, None, &map, height));
                }
                assert_rows(&codes, &map, table, height as i32);
            }
        }
    }
}
