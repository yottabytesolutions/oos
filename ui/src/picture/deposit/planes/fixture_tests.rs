// SPDX-License-Identifier: GPL-2.0-or-later

use crate::board::dho::hw::header::{dispatch_fixture, for_each_fixture, Fixture};
use crate::drawpipe::{host, route::RouteTrace, shutdown, stage::owner::Owner, Pipe, Stats};
use crate::picture::PerMap;
use crate::ui::lifecycle::test_wait::HANG_GUARD;
use crate::wave::Phosphor;
use std::time::Instant;

fn counts(ph: &Phosphor) -> u64 {
    (0..4)
        .flat_map(|channel| (0..ph.w * ph.h).map(move |i| ph.count(channel, i)))
        .fold(0xcbf2_9ce4_8422_2325, |hash, count| (hash ^ u64::from(count)).wrapping_mul(0x0100_0000_01b3))
}

fn replay(fixture: &Fixture) -> Option<(u64, Option<u64>)> {
    let view = fixture.view()?;
    let (model, (cols, rows), interp, zoom) = view.inputs();
    let (workers, prior_hits) = view.scheduling();
    let pipe = Pipe::start(workers, 0, Box::leak(Box::default()));
    pipe.publish_picture_view(model, (cols, rows), interp, Default::default(), zoom);
    let mut trace = RouteTrace::new(|_| {});
    trace.set_replay_prior_hits(prior_hits);
    dispatch_fixture(fixture, &pipe, Some(trace));
    host::finish_batch(&pipe, 0).expect("fixture DMA returned");
    let mut main = Phosphor::new(cols, rows);
    let mut detail = zoom.map(|z| {
        let (w, h) = z.size();
        Phosphor::new(w, h)
    });
    let accepted = Owner::default().compose_picture(
        &pipe,
        PerMap::new(&mut main, detail.as_mut()),
        &mut true,
        Stats { edges: &mut [0; 4], off: &mut (0.0, 0.0) },
        None,
    );
    assert_eq!(shutdown::close(&pipe, Instant::now() + HANG_GUARD), 0);
    assert!(accepted.frames > 0, "{} did not publish a picture", fixture.name());
    Some((counts(&main), detail.as_ref().map(counts)))
}

#[test]
#[ignore = "requires captured DMA batches in OOS_FIXTURES"]
fn captured_count_planes_match_baseline() {
    assert!(std::env::var_os("OOS_FIXTURES").is_some(), "select a captured fixture group");
    for_each_fixture(&mut |fixture| {
        assert_eq!(replay(fixture), Some(expected(fixture.name())), "{}", fixture.name());
    });
}

// Captured using the count planes on d60a21d after composing each plain fixture.
fn expected(name: &str) -> (u64, Option<u64>) {
    let index = name
        .strip_prefix("batch-")
        .and_then(|digits| digits.parse::<usize>().ok())
        .and_then(|index| index.checked_sub(4))
        .expect("plain fixtures are numbered from batch-004");
    BASELINE[index]
}

const BASELINE: [(u64, Option<u64>); 66] = [
    (0xf65f_d3f4_6f13_51d5, Some(0x1eed_0fcf_39dd_aab5)),
    (0xca25_ae63_ea32_2ded, Some(0x237c_1f57_ce74_874d)),
    (0xe0f8_d29e_0545_5125, Some(0x1fab_f6ce_bc62_8a6d)),
    (0x8f21_c249_37a5_e06d, Some(0xaaa7_7284_b853_5c05)),
    (0x652c_887d_b33f_cc85, None),
    (0x35e6_2f81_ec37_6595, None),
    (0xec83_f9fd_f237_0d3d, None),
    (0x0d67_de9e_2907_e3e5, None),
    (0x1f2d_b822_c1fa_8595, None),
    (0x18d6_0263_000e_f88d, None),
    (0x7a40_10d1_94f0_61a5, None),
    (0x91bf_9ed1_a845_fcd5, None),
    (0xf098_66cd_8ede_17ed, None),
    (0x3bf0_5615_b916_33a5, None),
    (0x18e9_25a5_fe0b_ddc5, None),
    (0x3499_0fdc_3100_6c85, None),
    (0xa2e6_e634_9991_45bd, None),
    (0x3852_915d_34b9_219d, None),
    (0xda47_530a_6641_bfdd, None),
    (0x3ba3_6d25_09f2_1365, None),
    (0xd959_ff5e_9149_f3a5, None),
    (0xbc15_3ef2_6e0e_0f55, None),
    (0xac40_b8e8_454a_7f45, None),
    (0x8db4_30a7_8909_88c5, None),
    (0x8a01_5285_1b66_86fd, None),
    (0x53b5_d33f_37e3_31bd, None),
    (0x3c9b_b0f9_bd77_4ee5, None),
    (0x8d87_0c0f_125d_7155, None),
    (0x832c_54ea_0623_06c5, None),
    (0x561f_cdb2_0548_220d, None),
    (0xe370_e081_daa0_b7b5, None),
    (0xcab7_94fb_52c8_d6e5, None),
    (0xa2b3_1b16_56fd_a7ed, None),
    (0xb213_98a1_0ac7_034d, None),
    (0xa835_60b5_f302_8f4d, None),
    (0x74d4_4bca_46a8_44bd, None),
    (0xbee8_d364_c855_47a5, None),
    (0x74f6_c65e_7b5e_4efd, None),
    (0x56a2_585b_a659_1d65, None),
    (0x9570_2107_e244_9555, None),
    (0x1f6d_2e0a_7bc4_3b15, None),
    (0xb3ca_8ee2_7591_2ac5, None),
    (0x79e7_e250_9902_cf7d, None),
    (0x651b_414d_a7d9_5b95, None),
    (0x0eb7_d30a_91ec_82ed, None),
    (0x7171_bd65_9b74_184d, None),
    (0x9c56_dd54_df21_18e5, None),
    (0xee34_6a3a_b1b1_01cd, None),
    (0x54ce_88e6_0f82_8585, None),
    (0x8062_dbdd_8222_b095, None),
    (0xcde9_70c2_d3ba_82f5, None),
    (0x8a62_e5fc_c6f7_52dd, None),
    (0xad2f_16e9_6be7_1d5d, None),
    (0x82b8_a326_6feb_da2d, None),
    (0x6d41_8aa3_bc5b_6fdd, Some(0x6c70_7e93_3fc0_3245)),
    (0x3e3d_ec2a_dc36_6c8d, Some(0x90a9_3fa6_55c8_9de5)),
    (0x2b1c_0531_179c_bfad, Some(0x8ba2_4e53_e74f_9f85)),
    (0xee3a_9596_8b01_f70d, Some(0x9441_66a2_c6cf_68ad)),
    (0xe4d3_8290_93e9_0f3d, None),
    (0x7e60_b2f4_2076_bf45, None),
    (0xf149_2be7_4634_254d, None),
    (0xde7a_4356_d7de_a865, None),
    (0x5907_b37d_d1a6_aef5, None),
    (0x2d95_ebc6_4be0_eb15, None),
    (0x07c3_8c37_bbf9_915d, None),
    (0xc81d_d3ef_9b51_57a5, None),
];
