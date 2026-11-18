// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

fn hardware() -> Ui {
    let mut ui = Ui::new(Source::Hw { shared: Default::default(), seen: 0 }, host::Host::headless());
    ui.m.meas.clear();
    ui.m.meas2.clear();
    ui.m.persist = PERSIST.len() - 1;
    ui
}

#[test]
fn repeated_present_record_observes_the_old_clear() {
    let mut ui = hardware();
    ui.commit_waveform_traces_at(
        vec![crate::store::WaveTrace { v: vec![-0.25, 0.25, -0.25, 0.25], dt: 1e-8, t0: 0.0, label: "record".into() }],
        "record",
        0,
    )
    .unwrap();
    ui.set_offline(true);
    ui.present_record(0);
    ui.step(0.04);
    change::reset(&mut ui);
    for _ in 0..2 {
        let before = change::counters(&ui).4;
        ui.present_record(0);
        ui.ph.channel_mut(0)[0] = 97;
        ui.phz.channel_mut(0)[0] = 97;
        change::begin_step(&mut ui, 0.04).unwrap();
        assert_eq!((ui.ph.maps()[0][0], ui.phz.maps()[0][0]), (0, 0));
        assert_eq!(change::counters(&ui).4, before, "present_record must observe its legacy clear");
    }
}

#[test]
fn offline_entry_with_all_channels_off_clears_stale_session_picture_once() {
    let mut ui = hardware();
    ui.m.run = RunState::Stop;
    for ch in &mut ui.m.ch {
        ch.on = false;
    }
    ui.step(0.04);
    ui.ph.channel_mut(0)[0] = 97;
    ui.phz.channel_mut(0)[0] = 97;
    ui.set_offline(true);
    assert_eq!(ui.ph.maps()[0][0], 97, "the accepted consumer owns the clear");
    change::begin_step(&mut ui, 0.04).unwrap();
    assert_eq!((ui.ph.maps()[0][0], ui.phz.maps()[0][0]), (0, 0));
    ui.ph.channel_mut(0)[0] = 97;
    change::begin_step(&mut ui, 0.04).unwrap();
    assert_eq!(ui.ph.maps()[0][0], 97, "entry request acts once");
}

#[test]
fn return_to_identical_retained_live_settings_clears_stale_picture_once() {
    let mut ui = hardware();
    ui.step(0.04);
    ui.remember_live();
    ui.session_is_sim = true;
    ui.ph.channel_mut(0)[0] = 97;
    ui.phz.channel_mut(0)[0] = 97;
    let before = ui.m.clone();
    ui.return_to_live();
    assert_eq!(ui.m, before, "retained settings produce no mapping diff");
    change::begin_step(&mut ui, 0.04).unwrap();
    assert_eq!((ui.ph.maps()[0][0], ui.phz.maps()[0][0]), (0, 0));
    ui.ph.channel_mut(0)[0] = 97;
    change::begin_step(&mut ui, 0.04).unwrap();
    assert_eq!(ui.ph.maps()[0][0], 97, "return request acts once");
}
