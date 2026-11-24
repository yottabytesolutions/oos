// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
#[test]
fn actual_dense_and_sparse_bands_keep_allocation_and_cancellation() {
    for format in [CountFormat::Narrow, CountFormat::Wide] {
        for dense in [false, true] {
            let mut a = DiffBand::bands((970, 448), [true; 4]);
            let mut b = DiffBand::bands((970, 448), [true; 4]);
            for (a, b) in a.iter_mut().zip(&mut b) {
                a.set_format(format);
                b.set_format(format);
                let (lo, hi) = if dense { (0, 447) } else { (447, 447) };
                for band in [&mut *a, &mut *b] {
                    let width = band.plane.band_cols;
                    match &mut band.wide {
                        Some((diff, _)) => {
                            for x in 0..width {
                                crate::fastdraw::diff_span(diff, width, x, lo, hi, 24);
                            }
                        }
                        None => {
                            for x in 0..width {
                                crate::fastdraw::diff_span(&mut band.plane.d, width, x, lo, hi, 24);
                            }
                        }
                    }
                    band.touch(lo..hi + 1);
                }
                let before = a.test_backing();
                let (peak, merged) = crate::system::test_allocator::own_high_water(|| a.merge_picture(b));
                assert!(merged);
                assert_eq!(peak, 0, "merge allocates no temporary backing or scratch");
                assert_eq!(a.test_backing(), before);
                let mut values = vec![0u32; 970];
                match &a.wide {
                    Some((diff, _)) => {
                        for row in 0..=448 {
                            for (sum, delta) in values.iter_mut().zip(&diff[row * 970..][..970]) {
                                *sum = sum.wrapping_add(*delta);
                            }
                            assert!(values.iter().all(|&v| v == if (lo..=hi).contains(&row) { 48 } else { 0 }));
                        }
                    }
                    None => {
                        let mut prefix = vec![0u16; 970];
                        for row in 0..=448 {
                            for (sum, delta) in prefix.iter_mut().zip(&a.plane.d[row * 970..][..970]) {
                                *sum = sum.wrapping_add(*delta);
                            }
                            assert!(prefix.iter().all(|&v| v == if (lo..=hi).contains(&row) { 48 } else { 0 }));
                        }
                    }
                }
            }
        }
    }
}
