// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

#[test]
fn mapping_publication_preserves_a_projection_held_by_a_reader() {
    use oos_hal::sync::lock;

    let pipe = Pipe::start(0, 0, fresh());
    let mut model = model(1e-6, false, false);
    let offset = model.ch[0].offset;
    let mut publish = |offset| {
        model.ch[0].offset = offset;
        pipe.set_view(Some(&model), 970, 448, Interp::Exact);
        let state = lock(&pipe.view);
        let view = state.view.as_ref().unwrap();
        assert_eq!(view.m.ch[0].offset, offset);
        std::sync::Arc::as_ptr(view)
    };
    publish(offset);
    let original = lock(&pipe.view).view.as_ref().unwrap().clone();
    let reusable = publish(offset + 0.125);
    let while_held = publish(offset + 0.25);
    assert_ne!(while_held, std::sync::Arc::as_ptr(&original), "a reader still owns the retired view");
    assert_eq!(std::sync::Arc::as_ptr(lock(&pipe.view).spare.as_ref().unwrap()), reusable);
    assert_eq!(publish(offset + 0.5), reusable, "the publisher reuses the unshared retired view");
    assert_eq!(original.m.ch[0].offset, offset);
}

#[test]
fn accepted_publication_refreshes_trigger_only_and_disables_for_single() {
    let _scope = test_scope::Scope::new();
    let mut ui = crate::ui::Ui::new(
        crate::wave::Source::Hw { shared: Default::default(), seen: 0 },
        crate::ui::host::Host::headless(),
    );
    ui.m.meas.clear();
    ui.m.meas2.clear();
    ui.step(0.04);
    let pipe = pipe().unwrap();
    assert!(pipe.accepting());
    let mapping = oos_core::change::MappingKey::of(&ui.m, Default::default());
    let before = oos_hal::sync::lock(&pipe.view).gen;
    ui.m.trig_level += 0.125;
    crate::ui::change::edited(&mut ui);
    ui.step(0.04);
    assert_eq!(mapping, oos_core::change::MappingKey::of(&ui.m, Default::default()));
    {
        let view = oos_hal::sync::lock(&pipe.view);
        assert_eq!(view.view.as_ref().unwrap().m.trig_level, ui.m.trig_level);
        assert_eq!(view.gen, before + 1, "one accepted worker refresh");
    }
    ui.step(0.04);
    assert_eq!(oos_hal::sync::lock(&pipe.view).gen, before + 1, "no late per-step refresh");
    ui.m.trig_mode = crate::model::TrigMode::Single;
    ui.step(0.04);
    assert!(!pipe.accepting());
    ui.m.trig_level += 0.125;
    ui.m.trig_mode = crate::model::TrigMode::Auto;
    ui.step(0.04);
    assert!(pipe.accepting());
    assert_eq!(oos_hal::sync::lock(&pipe.view).view.as_ref().unwrap().m.trig_level, ui.m.trig_level);
    ui.src = crate::wave::Source::Demo { t: 0.0 };
    ui.step(0.04);
    assert!(!pipe.accepting(), "leaving the hardware source disables its retained view");
}
