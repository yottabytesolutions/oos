// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

#[test]
fn accepted_publication_refreshes_trigger_only_and_disables_standard() {
    let _scope = test_scope::Scope::new();
    let mut ui = crate::ui::Ui::new(
        crate::wave::Source::Hw { shared: Default::default(), seen: 0 },
        crate::ui::host::Host::headless(),
    );
    ui.m.meas.clear();
    ui.m.meas2.clear();
    ui.m.draw_batch = true;
    ui.step(0.04);
    let pipe = pipe(Access::Existing).unwrap();
    assert!(pipe.accepting());
    let mapping = oos_core::change::MappingKey::of(&ui.m, Default::default());
    let before = oos_hal::sync::lock(&pipe.view).gen;
    ui.m.trig_level += 0.125;
    ui.mark_changed();
    ui.step(0.04);
    assert_eq!(mapping, oos_core::change::MappingKey::of(&ui.m, Default::default()));
    {
        let view = oos_hal::sync::lock(&pipe.view);
        assert_eq!(view.view.as_ref().unwrap().m.trig_level, ui.m.trig_level);
        assert_eq!(view.gen, before + 1, "one accepted worker refresh");
    }
    ui.step(0.04);
    assert_eq!(oos_hal::sync::lock(&pipe.view).gen, before + 1, "no late per-step refresh");
    ui.m.draw_batch = false;
    ui.step(0.04);
    assert!(!pipe.accepting());
    ui.m.trig_level += 0.125;
    ui.m.draw_batch = true;
    ui.step(0.04);
    assert!(pipe.accepting());
    assert_eq!(oos_hal::sync::lock(&pipe.view).view.as_ref().unwrap().m.trig_level, ui.m.trig_level);
    ui.src = crate::wave::Source::Demo { t: 0.0 };
    ui.step(0.04);
    assert!(!pipe.accepting(), "leaving the hardware source disables its retained view");
}
