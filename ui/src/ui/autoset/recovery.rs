// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

/// The smallest scale of `ch` with at least `vdiv` volts per division.
pub(super) fn scale_for(ch: &Channel, vdiv: f32) -> usize {
    VSCALES.iter().position(|&x| x * ch.probe >= vdiv).unwrap_or(VSCALES.len() - 1)
}

pub(in crate::ui) fn restart(ui: &mut Ui) {
    if let Some(a) = &mut ui.auto {
        a.stamp = 0;
        a.records = ui.records;
        a.since = ui.now;
        a.fits = false;
        if let Some(unit) = &mut a.unit {
            unit.since = ui.now;
        }
    }
    let _ = ui.src.hw_levels(None);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn foreign_clear_restarts_measure_unit_request_with_a_resolved_epoch() {
        let shared = crate::board::HwShared::default();
        let mut ui = Ui::for_tests(Source::Hw { shared: shared.clone(), seen: 0 });
        ui.auto_start();
        change::begin_step(&mut ui, 0.04).unwrap();
        change::finish_step(&mut ui);
        let old = ui.auto.as_ref().unwrap().stamp;
        assert!(ui.auto.as_ref().unwrap().unit.is_some());
        ui.clear_display();
        change::begin_step(&mut ui, 0.04).unwrap();
        assert_eq!(ui.auto.as_ref().unwrap().stamp, 0, "foreign pair must not accept unit answers");
        change::finish_step(&mut ui);
        change::begin_step(&mut ui, 0.04).unwrap();
        let epoch = ui.auto.as_ref().unwrap().stamp;
        assert!(epoch > old);
        let (token, wanted, _) = oos_hal::sync::lock(&shared).levels_want.unwrap();
        assert_eq!((token, wanted), (epoch, epoch));
        assert!(change::text(&ui).contains("stage_restart_autoset=1"));
        change::finish_step(&mut ui);
        for _ in 0..3 {
            ui.step(0.04);
        }
        assert!(change::text(&ui).contains("stage_restart_autoset=1"), "bounded to one restart per foreign boundary");
    }
}
