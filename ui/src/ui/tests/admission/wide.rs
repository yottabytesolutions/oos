// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::board::scripted::Batch as _;
use crate::drawpipe::RawBatch as _;
use crate::picture::deposit::{count::CountFormat, kernel, standard};

struct FlatBytes(Vec<u8>);
impl crate::drawpipe::RawBatch for FlatBytes {
    fn bytes(&self) -> &[u8] {
        &self.0
    }
    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any + Send> {
        self
    }
}

#[test]
fn legal_one_meg_depth_dots_reach_main_picture_and_cpu_layers_losslessly() {
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let mut ui = legal_ui();
    let switches = crate::sysconf::Switches { compress: false, ..Default::default() };
    let (input, frames, plan) = crate::board::scripted::draw_route_batch(&ui.m, &switches, 1);
    assert!(plan.contains("rate=156250000 depth=1000000 k=1") && plan.contains("plan_frames=1"), "{plan}");
    let (fb, decode, codes) = (input.frame_bytes(), input.decode(), input.codes());
    let mut bytes = input.into_bytes().bytes().to_vec();
    for word in bytes.as_chunks_mut::<2>().0 {
        word.copy_from_slice(&32768u16.to_le_bytes());
    }
    let mut frame = crate::board::Frame::default();
    decode(&bytes, &mut frame);
    let spacing = frame.dt / ui.m.time_per_div() as f64 * ui.ph.w as f64 / HDIV as f64;
    let projection = standard::projection(&ui.m, ui.ph.w, ui.ph.h, spacing);
    assert_eq!(projection.format, CountFormat::Wide);
    assert_eq!(standard::dot_hit_bound(spacing), 2884);
    let expected = reference(&ui, &frame, &projection);
    let pipe = crate::drawpipe::pipe(&switches).unwrap();
    let identity = oos_core::change::Identity::default();
    pipe.publish_view(&ui.m, ui.ph.w, ui.ph.h, switches.interp, identity);
    assert!(pipe.read_limit(identity, Some((156_250_000, 1))).unwrap() >= frames as u32);
    assert!(pipe.feed_with(Box::new(FlatBytes(bytes)), fb, 0..frames, identity, decode, codes, None).is_none());
    let _returned = crate::drawpipe::host::finish_batch(pipe, 0).unwrap();
    let got = pipe.accept_into(
        &mut ui.ph,
        &mut true,
        crate::drawpipe::Stats { edges: &mut [0; 4], off: &mut (0.0, 0.0) },
        None,
    );
    assert_eq!(got.frames, 1);
    assert!(ui.ph.is_wide());
    for (c, map) in expected.iter().enumerate() {
        assert_eq!((0..map.len()).map(|i| ui.ph.count(c, i)).collect::<Vec<_>>(), *map);
    }
    assert_eq!(crate::drawpipe::counters().read().2, 0);
    verify_render(&mut ui);
}

fn legal_ui() -> Ui {
    let mut ui = Ui::new(Source::Demo { t: 0.0 }, host::Host::headless());
    ui.m.win_layout = WinLayout::Quad;
    ui.m.strip_dock = StripDock::Left;
    ui.m.cursor = CursorMode::Time;
    ui.m.bar_compact = true;
    ui.m.meas = Ui::default_meas();
    ui.stats_on = true;
    ui.workspace.measure_docked = true;
    ui.workspace.measure_float = None;
    ui.m.text_size = TextSize::Larger;
    ui.m.dots = DotsMode::On;
    ui.m.tscale = tscales().iter().position(|&t| (t - 500e-6).abs() < 1e-10).unwrap();
    ui.m.mdepth = mdepths().iter().position(|d| d.1 == 1_000_000).unwrap();
    for ch in &mut ui.m.ch {
        ch.on = true;
    }
    ui.relayout();
    assert_eq!(ui.ph.w, 271);
    ui
}

fn reference(ui: &Ui, frame: &crate::board::Frame, projection: &kernel::Projection) -> [Vec<u32>; NCH] {
    let mut expected: [Vec<u32>; NCH] = std::array::from_fn(|_| vec![0; ui.ph.w * ui.ph.h]);
    for (c, map) in expected.iter_mut().enumerate() {
        let p = kernel::raw_map(projection, c, frame.pre as f64).unwrap();
        for (i, &v) in frame.ch[c].iter().enumerate() {
            let x = ((i as f64 - p.ti) * p.x_per_sample + p.x0).floor();
            let y = (v * p.y_per_volt + p.y0).round();
            if x >= 0.0 && x < ui.ph.w as f64 && y >= 0.0 && y < ui.ph.h as f32 {
                map[y as usize * ui.ph.w + x as usize] += 24;
            }
        }
        assert!(map.iter().copied().max().unwrap() > 65_535);
    }
    expected
}

fn verify_render(ui: &mut Ui) {
    ui.batch_picture.frames = 1;
    let before = ui.ph.channel_max(0);
    assert_eq!(ui.grade_refs(), (false, false), "Min must retain exact wide counts");
    let area = Rect::new(0, 0, ui.ph.w as i32, ui.ph.h as i32);
    for graded in [false, true] {
        ui.m.color_grade = graded;
        ui.grade_refs();
        let mut cpu = Canvas::new(area.w, area.h);
        let mut under = Canvas::new(area.w, area.h);
        let mut over = Canvas::new(area.w, area.h);
        ui.traces(&mut Layers { under: &mut cpu, over: None, jobs: Vec::new() }, area, false, true);
        let mut layers = Layers { under: &mut under, over: Some(&mut over), jobs: Vec::new() };
        ui.traces(&mut layers, area, false, true);
        let job = &layers.jobs[0];
        verify_indices(ui, job, graded);
        assert!(job.indexed.as_ref().unwrap().iter().map(Vec::capacity).sum::<usize>() <= NCH * ui.ph.w * ui.ph.h);
        assert!(cpu.px.iter().any(|&p| p != 0));
    }
    assert_eq!(ui.ph.channel_max(0), before);
}

fn verify_indices(ui: &Ui, job: &TraceJob, graded: bool) {
    let maps = job.indexed.as_ref().unwrap();
    let pixels = ui.ph.w * ui.ph.h;
    if graded {
        let (lut, grade_shift) = ui.grade_lut(ui.vmax_cg);
        assert_eq!(job.show, [true, false, false, false]);
        assert_eq!(maps[0].len(), pixels);
        for (i, &encoded) in maps[0].iter().enumerate() {
            let sum: u32 = (0..NCH).map(|c| ui.ph.count(c, i)).sum();
            let encoded = u32::from(encoded);
            assert_eq!(encoded != 0, sum != 0);
            assert_eq!(lut[(encoded >> job.shift[0]).min(1023) as usize], lut[(sum >> grade_shift).min(1023) as usize]);
        }
    } else {
        for (c, map) in maps.iter().enumerate() {
            let (_, trace_shift, _) = ui.trace_lut(ui.vmax[c]);
            assert_eq!(map.len(), pixels);
            for (i, &encoded) in map.iter().enumerate() {
                let value = ui.ph.count(c, i);
                let encoded = u32::from(encoded);
                assert_eq!(encoded != 0, value != 0);
                assert_eq!((encoded >> job.shift[c]).min(1023), (value >> trace_shift).min(1023));
            }
        }
    }
}

#[test]
fn wide_grade_reference_uses_exact_nonoverflowing_follow_arithmetic() {
    let mut per_frame = 1;
    assert_eq!(picture_ref(u32::MAX, &mut per_frame, 1, 0), ((u64::from(u32::MAX) * 7 + 1) / 8) as u32);
    assert_eq!(picture_ref(0, &mut per_frame, 69_216, 1), 69_216);
}
