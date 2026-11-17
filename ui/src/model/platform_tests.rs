// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;
use oos_core::config::{opt_src_step_in, src_step_in};

// Unit 2026-11-14 (deepraw): 100M with CH1 shown and the trigger on CH2 ran at 62.5M while the menu said 100M.
// Break it catches: a label that hides the cap, a cap blamed on the trigger when the shown channels cause it, a
// label on a depth that fits.
#[test]
fn depth_label_shows_the_applied_depth() {
    let mut m = Model::default();
    let at = |d: u64| mdepths().iter().position(|x| x.1 == d).unwrap();
    for c in 0..NCH {
        m.ch[c].on = c == 0;
    }
    (m.trig_type, m.trig_src, m.mdepth) = (TrigType::Edge, 1, at(100_000_000));
    assert_eq!(m.depth_label(), "100M \u{2192} 62.5M (trig CH2)");
    m.trig_src = 0;
    assert_eq!((m.depth_label(), m.depth_cap()), ("100M".to_string(), None));
    m.ch[1].on = true;
    assert_eq!(m.depth_label(), "100M \u{2192} 62.5M");
    m.mdepth = at(50_000_000);
    assert_eq!(m.depth_label(), "50M");
    m.mdepth = 0;
    assert_eq!(m.depth_label(), "Auto");
}

#[test]
fn source_steps_skip_missing_channels() {
    // 2-channel unit: CH1, CH2, then (math sources) M1.. at NCH..
    assert_eq!(src_step_in(0, NCH, 1, 2), 1);
    assert_eq!(src_step_in(1, NCH, 1, 2), 0);
    assert_eq!(src_step_in(0, NCH, -1, 2), 1);
    assert_eq!(src_step_in(1, NCH + 2, 1, 2), NCH);
    assert_eq!(src_step_in(NCH + 1, NCH + 2, 1, 2), 0);
    // a stale CH4 source moves from CH1
    assert_eq!(src_step_in(3, NCH, 1, 2), 1);
    // 4 channels: plain cycling
    assert_eq!(src_step_in(3, NCH, 1, 4), 0);
    assert_eq!(src_step_in(2, NCH + 1, 1, 4), 3);
    assert_eq!((opt_src_step_in(2, 1, 2), opt_src_step_in(0, -1, 2), opt_src_step_in(4, 1, 4)), (0, 2, 0));
}

#[test]
fn four_channel_unit_keeps_everything() {
    // the default platform (host tests) is the DHO924S
    let mut m = Model::default();
    m.ch[3].on = true;
    m.dec_src = 3;
    assert!(!oos_core::change::normalize(&mut m, &config()));
    assert!(m.ch[3].on && m.dec_src == 3);
    assert_eq!(ch_names().len(), 4);
}

#[test]
fn every_generator_keeps_its_settings() {
    let mut m = Model::default();
    assert_eq!(m.gen_read(), GenCh::STANDARD);
    m.afg_freq = 5e3;
    m.afg_on = true;
    if NGEN < 2 {
        m.gen_select(1);
        assert_eq!((m.gen_sel, m.afg_freq), (0, 5e3), "no second generator on this board");
        return;
    }
    m.gen_select(1);
    assert_eq!(m.gen_read(), GenCh::STANDARD);
    m.afg_freq = 7e3;
    assert_eq!(m.gen_ch(0).freq, 5e3);
    m.gen_select(0);
    assert!(m.afg_freq == 5e3 && m.afg_on);
    assert_eq!(m.gen_ch(1).freq, 7e3);
}

#[test]
fn a_setup_of_another_board_fits_this_one() {
    let mut m = Model::default();
    assert!(!oos_core::change::normalize(&mut m, &config()));
    if !board().couplings().contains(&Coupling::Dc50) {
        m.ch[1].coupling = Coupling::Dc50;
        assert!(oos_core::change::normalize(&mut m, &config()));
        assert_eq!(m.ch[1].coupling, Coupling::Dc);
    }
}

#[test]
fn the_boards_lists_are_usable() {
    let d = mdepths();
    assert_eq!(d[0].1, 0, "Auto first");
    assert!(d.windows(2).all(|w| w[0].1 < w[1].1), "ascending");
    assert_eq!(d[d.len() - 1].1, oos_hal::model::Limits::standard().max_mem[0], "the list ends at the largest depth");
    assert!(board().couplings().contains(&Coupling::Dc), "the fallback of normalize");
    assert!(n_ch() <= NCH && gen_count() <= NGEN);
}
