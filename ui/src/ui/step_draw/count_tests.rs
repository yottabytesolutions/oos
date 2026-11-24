// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;
#[test]
fn fresh_membership_excludes_retained_redraw_stop_single_and_disabled_observer() {
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let mut ui = Ui::new(Source::Hw { shared: Default::default(), seen: 0 }, host::Host::headless());
    ui.m.draw_batch = true;
    ui.m.acq = AcqMode::Normal;
    ui.m.trig_type = TrigType::Edge;
    ui.m.trig_mode = TrigMode::Normal;
    ui.m.run = RunState::Run;
    ui.m.tscale = tscales().iter().position(|&v| (v - 1e-6).abs() < 1e-10).unwrap();
    let counts = crate::drawpipe::counters().timing.picture_counts();
    for (records, redrawn) in [(0, 8), (3, 9), (0, 0), (2, 0)] {
        account_picture(&ui, records, Composition { merged: records as u64, redrawn, ..Default::default() });
        counts.text();
        counts.text();
    }
    assert!(counts.text().contains("eligible=4 fresh=2 empty=2 first_records=5"));
    ui.m.run = RunState::Stop;
    account_picture(&ui, 9, Composition::default());
    ui.m.run = RunState::Run;
    ui.m.trig_mode = TrigMode::Single;
    account_picture(&ui, 9, Composition::default());
    ui.m.trig_mode = TrigMode::Normal;
    crate::drawpipe::counters().diagnostics_disabled.store(true, std::sync::atomic::Ordering::Relaxed);
    account_picture(&ui, 9, Composition::default());
    assert!(counts.text().contains("eligible=4 fresh=2 empty=2 first_records=5"));
}
