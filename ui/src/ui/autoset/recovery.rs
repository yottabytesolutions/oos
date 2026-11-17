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
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn foreign_clear_restarts_record_stage_with_a_resolved_epoch() {
        let shared = crate::board::HwShared::default();
        let mut ui = Ui::for_tests(Source::Hw { shared: shared.clone(), seen: 0 });
        ui.auto_start();
        ui.step(0.04);
        let old = ui.auto.as_ref().unwrap().stamp;
        ui.clear_display();
        ui.step(0.04);
        assert_eq!(ui.auto.as_ref().unwrap().stamp, 0, "foreign pair must not accept records");
        ui.step(0.04);
        let epoch = ui.auto.as_ref().unwrap().stamp;
        assert!(epoch > old);
        let a = ui.auto.as_ref().unwrap();
        assert_eq!(a.records, ui.records);
        assert!(!a.fits);
        assert!(change::text(&ui).contains("stage_restart_autoset=1"));
        for _ in 0..3 {
            ui.step(0.04);
        }
        assert!(change::text(&ui).contains("stage_restart_autoset=1"), "bounded to one restart per foreign boundary");
    }
}
