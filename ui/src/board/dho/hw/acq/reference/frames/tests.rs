// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::board::dho::cores::Cores;
use crate::system::test_allocator::count_own;
use oos_hal::dho::seq;

#[test]
fn warmed_profiles_reuse_decoder_callbacks_across_batch_sizes() {
    let full = Settings { depth: 14_064, tdiv: 1e-6, window: None, ..Settings::default() };
    let p = hz::prep(full.mask, seq::tdiv_fs(full.tdiv), full.depth, 0);
    let mut peak = full.clone();
    let reduced = peak.reduce_capture(true, 2, true).unwrap();
    let mut decoders = Decoders::default();
    let (full_decode, _) = decoders.for_profile(&full, p, HeaderProbe::Off);
    let (peak_decode, peak_codes) = decoders.for_profile(&peak, reduced, HeaderProbe::Off);
    let (allocations, ()) = count_own(|| {
        for frames in 1..9 {
            peak.frames = frames;
            let (decode, codes) = decoders.for_profile(&peak, reduced, HeaderProbe::Off);
            assert!(Arc::ptr_eq(&decode, &peak_decode));
            assert!(Arc::ptr_eq(codes.as_ref().unwrap(), peak_codes.as_ref().unwrap()));
            let (decode, codes) = decoders.for_profile(&full, p, HeaderProbe::Off);
            assert!(Arc::ptr_eq(&decode, &full_decode) && codes.is_none());
        }
    });
    assert_eq!(allocations, 0);
}

#[test]
fn retained_reference_decode_allocates_one_vector_per_channel_and_one_arc() {
    use crate::picture::deposit::admission::Share;
    use std::time::Duration;

    let scope = crate::drawpipe::test_scope::Scope::new();
    let _intake = scope.install_intake();
    let pipe = crate::drawpipe::pipe().unwrap();
    let id = Identity::default();
    pipe.publish_view(&crate::model::Model::default(), 128, 128, crate::sparse::Interp::Exact, id);
    let budget = 64 << 20;
    pipe.fund_taps(Share::checked(budget, 0, budget, 256, Arc::new(())).unwrap(), Duration::from_millis(1)).unwrap();
    for mask in [1u32, 3, 7, 15] {
        let s = Settings { mask, depth: 14_064, tdiv: 1e-6, window: None, ..Settings::default() };
        let p = hz::prep(mask, seq::tdiv_fs(s.tdiv), s.depth, 0);
        let mut bytes = vec![0; seq::frame_bytes(&s, &p)];
        for (i, word) in bytes.chunks_exact_mut(p.ratio as usize * 2).enumerate() {
            for lane in word.as_chunks_mut::<2>().0 {
                *lane = (32768u16 + (i % 8) as u16 * 10).to_le_bytes();
            }
        }
        let mut cores = Cores::default();
        assert!(super::super::super::super::batch::deposit_plan(&s, &p, id, true).0.is_some());
        let tap = pipe.reserve_plan(id).unwrap();
        for reservation in [None, Some(&tap)] {
            let mut decode = |seq| {
                decode_reference(&bytes, (&s, &p, HeaderProbe::Off), Some(&mut cores), (id, seq, reservation))
                    .unwrap()
                    .0
            };
            let prior = decode(1);
            let (allocations, next) = count_own(|| decode(2));
            assert_eq!(allocations, mask.count_ones() as usize + 1, "mask {mask:#x}");
            assert_eq!((prior.seq, next.seq), (1, 2));
        }
    }
}
