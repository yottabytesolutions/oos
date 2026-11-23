// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;

fn reference(count: usize) -> Ui {
    let mut ui = Ui::for_tests(Source::Demo { t: 0.0 });
    ui.m.run = RunState::Stop;
    ui.m.meas_full_record = true;
    ui.m.meas.clear();
    ui.m.meas2.clear();
    ui.refs[0] = Some(RefWave {
        src: 0,
        color: 0,
        label: "row-test".into(),
        t0: 0.001,
        dt: 1e-9,
        v: ((0..count)
            .map(|i| {
                let k = i % 400;
                match k {
                    0..=99 => -0.5,
                    100..=199 => -0.5 + (k - 100) as f32 / 99.0,
                    200..=299 => 0.5,
                    _ => 0.5 - (k - 300) as f32 / 99.0,
                }
            })
            .collect::<Vec<_>>())
        .into(),
        vpd: 0.2,
        offset: 0.0,
        on: true,
    });
    ui
}

fn custom(ui: &mut Ui, kind: Meas, low: f64, high: f64) {
    let id = WaveId::Ref(0);
    ui.m.meas.push((kind, id));
    ui.meas_own.insert((kind, id), MeasOwn { refs_set: true, low, high, ..MeasOwn::shared(&ui.m) });
}

fn wait(ui: &mut Ui, kinds: &[Meas]) -> Vec<f64> {
    let end = crate::ui::lifecycle::test_wait::Wait::new("custom row measurement values");
    loop {
        ui.poll_reference_measurements();
        let values: Vec<_> = kinds.iter().map(|&kind| ui.meas_number(WaveId::Ref(0), kind)).collect();
        if values.iter().all(|v| v.is_finite()) {
            return values;
        }
        end.check();
        end.ui(ui);
    }
}

#[test]
fn custom_rows_keep_physical_extremum_time_and_scpi_row_parity() {
    let mut ui = reference(800);
    custom(&mut ui, Meas::TVmax, 0.2, 0.8);
    let id = WaveId::Ref(0);
    let time = ui.meas_number(id, Meas::TVmax);
    assert!((time - (0.001 + 199e-9)).abs() < 1e-14, "{time}");
    assert_eq!(ui.fact_meas(Meas::TVmax, id), time);
    assert_eq!(ui.meas_rows()[0].v, time);
    assert_eq!(ui.set_cell(Meas::TVmax, 0), time);
    assert!(!ui.meas_rows()[0].computing);
    assert_eq!((ui.records, ui.wfm_total), (0, 0));
}

#[test]
fn large_distinct_row_levels_share_one_worker_without_cache_thrash() {
    let mut ui = reference(600_000);
    custom(&mut ui, Meas::Rise, 0.1, 0.9);
    custom(&mut ui, Meas::Fall, 0.2, 0.8);
    assert!(ui.meas_number(WaveId::Ref(0), Meas::Rise).is_nan());
    assert!(ui.meas_number(WaveId::Ref(0), Meas::Fall).is_nan());
    assert!(ui.meas_rows().iter().all(|row| row.computing));
    let values = wait(&mut ui, &[Meas::Rise, Meas::Fall]);
    assert!((values[0] - 79.2e-9).abs() < 3e-9, "{values:?}");
    assert!((values[1] - 59.4e-9).abs() < 3e-9, "{values:?}");
    let submitted = ui.azr.ref_measure_worker.borrow().as_ref().unwrap().submitted;
    for _ in 0..100 {
        assert_eq!(ui.meas_number(WaveId::Ref(0), Meas::Rise), values[0]);
        assert_eq!(ui.fact_meas(Meas::Fall, WaveId::Ref(0)), values[1]);
        assert!(ui.meas_rows().iter().all(|row| !row.computing));
    }
    assert_eq!(ui.azr.ref_measure_worker.borrow().as_ref().unwrap().submitted, submitted);
    assert_eq!(ui.azr.own_measure.borrow().cached.len(), 2);
    assert_eq!((ui.m.run, ui.records, ui.wfm_total), (RunState::Stop, 0, 0));
}

#[test]
fn pending_row_rejects_changed_gate_levels_and_replaced_reference() {
    let mut ui = reference(600_000);
    custom(&mut ui, Meas::Rise, 0.1, 0.9);
    assert!(ui.meas_number(WaveId::Ref(0), Meas::Rise).is_nan());
    let old = ui.azr.own_measure.borrow().pending[0].key;
    ui.m.meas_full_record = false;
    ui.m.tscale = tscales().iter().position(|&v| (v - 50e-6).abs() < 1e-10).unwrap();
    ui.m.delay = 0.0013;
    ui.meas_own.get_mut(&(Meas::Rise, WaveId::Ref(0))).unwrap().low = 0.2;
    ui.meas_own.get_mut(&(Meas::Rise, WaveId::Ref(0))).unwrap().high = 0.8;
    let original = ui.refs[0].as_ref().unwrap().v.clone();
    ui.refs[0].as_mut().unwrap().v = original;
    ui.workflow_epoch = ui.workflow_epoch.wrapping_add(1);
    let value = wait(&mut ui, &[Meas::Rise])[0];
    assert!((value - 59.4e-9).abs() < 3e-9, "{value}");
    let state = ui.azr.own_measure.borrow();
    assert!(!state.cached.iter().any(|cache| cache.key == old), "stale result entered the cache");
    assert!(state.cached.iter().all(|cache| !cache.key.analysis.full && cache.key.analysis.low == 0.2f64.to_bits()));
}

#[test]
fn evicting_edge_arrays_preserves_scalar_values_without_rescanning() {
    let mut ui = reference(800);
    let kinds = [Meas::Vpp, Meas::Vrms, Meas::Freq, Meas::Rise, Meas::Fall, Meas::TVmax];
    for (i, &kind) in kinds.iter().enumerate() {
        custom(&mut ui, kind, 0.1 + i as f64 * 0.01, 0.9);
    }
    let first = ui.meas_number(WaveId::Ref(0), kinds[0]);
    for &kind in &kinds[1..] {
        assert!(ui.meas_number(WaveId::Ref(0), kind).is_finite());
    }
    let state = ui.azr.own_measure.borrow();
    assert_eq!(state.cached.len(), kinds.len());
    assert_eq!(state.cached.iter().filter(|cache| cache.analysis.is_some()).count(), ANALYSIS_LIMIT);
    assert!(state.cached.front().unwrap().analysis.is_none());
    drop(state);
    assert_eq!(ui.meas_number(WaveId::Ref(0), kinds[0]), first);
    assert!(ui.azr.ref_measure_worker.borrow().is_none(), "scalar read must not recreate evicted edges");
    assert_eq!(ui.azr.own_measure.borrow().cached.len(), kinds.len());
}

#[test]
fn unfinished_query_expires_with_its_value_unavailable() {
    let mut ui = reference(600_000);
    let (thread, _jobs, _results) = crate::ui::lifecycle::worker::Worker::manual();
    *ui.azr.ref_measure_worker.borrow_mut() = Some(RefMeasureWorker { thread, submitted: 0, own_turn: false });
    custom(&mut ui, Meas::Rise, 0.1, 0.9);
    assert!(ui.meas_number(WaveId::Ref(0), Meas::Rise).is_nan());
    let start = std::time::Instant::now();
    let now = std::rc::Rc::new(std::cell::Cell::new(start));
    let waits = std::rc::Rc::new(std::cell::Cell::new(0));
    let read = now.clone();
    ui.host.runtime_mut().query_time.now = std::rc::Rc::new(move |_| read.get());
    let count = waits.clone();
    ui.host.runtime_mut().query_time.wait = std::rc::Rc::new(move |_| {
        assert_eq!(count.replace(count.get() + 1), 0, "query must stop at QUERY_WAIT");
        now.set(start + QUERY_WAIT);
    });
    ui.measurement_settle();
    assert_eq!(waits.get(), 1);
    assert!(ui.measurement_busy(), "the submitted job still has no result");
    assert!(ui.meas_number(WaveId::Ref(0), Meas::Rise).is_nan());
    assert!(ui.azr.own_measure.borrow().cached.is_empty());
    assert_eq!(ui.azr.ref_measure_worker.borrow().as_ref().unwrap().submitted, 1);
}
