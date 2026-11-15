// SPDX-License-Identifier: GPL-2.0-or-later

use crate::HEAP;
use oos_core::{
    change::{Outbox, Shadow, UiAcq},
    config::Config,
    model::{Meas, Model},
};

pub fn check() {
    let config = Config::new(4, 1, true, [true; 4], &[("Auto", 0)], [1e9; 3], [[1_000_000; 3]; 2]);
    let mut model = Model::default();
    warm(&mut model);
    let mut shadow =
        Shadow::new(&model, UiAcq::default(), &config).expect("bounded test fixture must fit the shadow and outbox");
    let mut outbox = Outbox::default();
    let mut previous = model.clone();
    for step in 0..100 {
        change(&mut model, step);
        outbox.capture(&previous, &model).expect("bounded test fixture must fit the shadow and outbox");
        outbox.apply(&mut previous);
        outbox.clear();
        shadow
            .observe(&model, UiAcq::default(), &[], &config)
            .expect("bounded test fixture must fit the shadow and outbox");
    }
    let naive_probe = std::env::var_os("OOS_NAIVE_CHANGE_PROBE").is_some();
    let before = HEAP.allocations();
    for step in 0..1000 {
        if naive_probe {
            std::hint::black_box(model.clone());
        }
        shadow
            .observe(std::hint::black_box(&model), UiAcq::default(), &[], &config)
            .expect("bounded test fixture must fit the shadow and outbox");
        change(&mut model, step);
        outbox.capture(&previous, &model).expect("bounded test fixture must fit the shadow and outbox");
        outbox.apply(&mut previous);
        outbox.clear();
        shadow
            .observe(std::hint::black_box(&model), UiAcq::default(), &[], &config)
            .expect("bounded test fixture must fit the shadow and outbox");
    }
    let reused = HEAP.allocations() - before;
    assert_eq!(reused, 0, "unchanged and warmed bounded snapshot/normalize updates must not allocate or reallocate");
    let before = HEAP.allocations();
    for _ in 0..10 {
        std::hint::black_box(model.clone());
    }
    let naive = HEAP.allocations() - before;
    assert!(naive > 0, "the allocator must reject naive derived Model clones");
    let (previous, scratch) = shadow.bytes();
    assert!(previous < 64 * 1024 && scratch < 64 * 1024);
    println!("snapshot allocations reused={reused} naive={naive} previous_bytes={previous} scratch_bytes={scratch}; backend=process-wide System successful alloc/alloc_zeroed/realloc");
}
fn warm(model: &mut Model) {
    for s in model
        .ch_label
        .iter_mut()
        .chain(&mut model.math_expr)
        .chain(&mut model.view_slots)
        .chain(std::iter::once(&mut model.bode_addr))
    {
        *s = "w".repeat(64);
    }
    model.meas = vec![(Meas::Vavg, 0); 16];
    model.meas2 = vec![(0, 0, 1); 16];
    for z in &mut model.zones {
        z.pts = vec![(0.0, 1.0); 16];
    }
}
fn change(model: &mut Model, step: usize) {
    for s in model.ch_label.iter_mut().chain(&mut model.view_slots).chain(std::iter::once(&mut model.bode_addr)) {
        s.clear();
        s.push_str(if step.is_multiple_of(2) { "short" } else { "warmed change" });
    }
    if !step.is_multiple_of(3) {
        for s in &mut model.math_expr {
            s.clear();
            s.push_str(if step.is_multiple_of(2) { "short" } else { "warmed change" });
        }
    }
    model.meas.truncate(8);
    model.meas.resize(if step.is_multiple_of(2) { 16 } else { 12 }, (Meas::Vavg, 0));
    model.meas2.truncate(8);
    model.meas2.resize(if step.is_multiple_of(2) { 16 } else { 12 }, (0, 0, 1));
    for z in &mut model.zones {
        z.pts.truncate(8);
        z.pts.resize(if step.is_multiple_of(2) { 16 } else { 12 }, (0.0, 1.0));
    }
    model.ch[0].offset = if step.is_multiple_of(2) { 1e6 } else { -1e6 };
}
