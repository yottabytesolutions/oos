// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

#[test]
fn accepted_view_change_defers_one_refresh_to_analysis() {
    let mut ui = fixture(AView::Eye, 10_000, 2e-7, |k| if k % 86 < 43 { -1.0 } else { 1.0 });
    ui.m.run = RunState::Stop;
    change::reset(&mut ui);
    calculate(&mut ui);
    assert_eq!(ui.azr.eye.as_ref().unwrap().records, 1);
    ui.m.eye_rate = 0.0;
    change::begin_step(&mut ui, 0.0).unwrap();
    assert!(ui.azr.eye.is_none(), "the consumer resets before computation");
    assert!(ui.azr.work.pending);
    assert!(change::acquisition::record(&ui, ui.last[0].as_ref().unwrap()), "retained current record");
    ui.meas_poll();
    assert!(
        ui.azr.eye.is_some(),
        "pending={} view={:?} rate={} status={:?}",
        ui.azr.work.pending,
        ui.m.view,
        ui.m.eye_rate,
        ui.azr.page.status
    );
    assert_eq!(ui.azr.eye.as_ref().unwrap().records, 1, "one refresh in the analysis phase");
    ui.meas_poll();
    assert_eq!(ui.azr.eye.as_ref().unwrap().records, 1, "an idle poll cannot count the record twice");
}
