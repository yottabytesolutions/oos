// SPDX-License-Identifier: GPL-2.0-or-later

use crate::{
    board::{board, Board, Frame, HwShared},
    model::*,
    ui::Ui,
    wave::Source,
};
use oos_hal::sync::lock;
use std::{hint::black_box, sync::Arc};

type Observer<'a> = dyn FnMut(&str, usize, Option<&mut dyn FnMut()>, &mut dyn FnMut()) + 'a;

fn fixture(n: usize, queued_raw: bool) -> (Ui, HwShared, Arc<Frame>) {
    let shared = HwShared::default();
    let mut ui = Ui::new(Source::Hw { shared: shared.clone(), seen: 0 }, crate::ui::host::Host::headless());
    ui.m.run = RunState::Run;
    ui.m.meas = if queued_raw { vec![(Meas::Vavg, WaveId::Ch(0))] } else { Vec::new() };
    ui.m.meas2.clear();
    ui.m.tscale = tscales().iter().position(|&t| (t - 1e-3).abs() < 1e-9).expect("1ms timebase exists");
    ui.m.mdepth = board().depths().iter().position(|d| d.1 == 10_000_000).expect("10M depth exists");
    for c in &mut ui.m.ch {
        c.on = true;
    }
    ui.step(1.0 / 30.0);
    let id = crate::ui::change::acquisition::identity(&ui);
    let frame = Arc::new(Frame {
        seq: 2,
        dt: 0.032 / n as f64,
        pre: n / 2,
        ch: std::array::from_fn(|_| vec![1.0; n]),
        envelope: true,
        acq: (312.5e6, n * 16),
        cfg: id.gen(),
        stamp: id.epoch(),
        ..Default::default()
    });
    (ui, shared, frame)
}

pub fn run(mut sample: impl FnMut(&str, usize, Option<&mut dyn FnMut()>, &mut dyn FnMut())) {
    for n in [625_000, 125_000] {
        steps(n, false, &mut sample);
    }
    steps(125_000, true, &mut sample);
    readers(&mut sample);
}

fn steps(n: usize, queued_raw: bool, sample: &mut Observer<'_>) {
    let (mut ui, shared, frame) = fixture(n, queued_raw);
    let raw = Arc::new(Frame {
        seq: 1,
        envelope: false,
        ch: std::array::from_fn(|_| vec![1.0; crate::compress::MAX_DEPTH as usize + 1]),
        ..(*frame).clone()
    });
    lock(&shared).read_geometry.deep_ok = true;
    let name = if queued_raw {
        "queued_raw_before_pair"
    } else if n == 625_000 {
        "deep_step"
    } else {
        "shallower_pairs"
    };
    sample(name, n * size_of::<f32>(), None, &mut || {
        let mut g = lock(&shared);
        g.frame = frame.clone();
        if queued_raw {
            g.queue.push_back(raw.clone());
        }
        g.queue.push_back(frame.clone());
        drop(g);
        ui.step(1.0 / 30.0);
        let retained = &ui.last[0].as_ref().expect("queued pair accepted").raw.as_ref().expect("retains raw").frame;
        assert!(Arc::ptr_eq(&frame, retained));
    });
}

fn readers(sample: &mut Observer<'_>) {
    let (mut ui, _, frame) = fixture(125_000, false);
    let raw = Arc::new(Frame { envelope: false, ..(*frame).clone() });
    ui.last[0] = Some(crate::wave::hw_record(&ui.m, raw.clone()));
    ui.azr.watch = true;
    let record = ui.last[0].as_ref().expect("raw reader fixture has a record");
    let bytes = std::mem::size_of_val(ui.meas_slice(record, 0).0);
    let r = record.raw.as_ref().expect("raw reader fixture retains its window");
    let (lo, hi) = (r.i0, r.i1);
    let refs: [&[f32]; NCH] = std::array::from_fn(|c| &raw.ch[c][lo..hi]);
    let mut reference = || {
        // Identify owns DSP scratch. Count its legitimate allocations independently,
        // then require both live handlers to add no sample copies on top.
        black_box(oos_dsp::identify::identify(&refs, raw.dt));
        black_box(oos_dsp::identify::identify(&refs, raw.dt));
    };
    sample("raw_input_readers", bytes, Some(&mut reference), &mut || {
        for c in 0..NCH {
            black_box(ui.math_source_record(c, 0));
            let (v, dt) = crate::ui::autoset::samples::search(&ui, c, 0.0);
            assert_eq!(v.as_ptr(), raw.ch[c].as_ptr());
            assert_eq!(dt, raw.dt);
        }
        black_box(ui.identify_text());
        ui.now += 2.1;
        ui.protocol_watch();
    });
}
