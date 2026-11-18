// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::board::{
    dho::switches::HeaderProbe,
    scripted::{self, Batch as _},
};
use crate::{model::Model, sparse::Interp, wave::Phosphor};

#[test]
fn normal_lines_four_channel_admission_preserves_the_real_read_plans() {
    for staged in [false, true] {
        check_normal_lines_plans(staged);
    }
}

fn check_normal_lines_plans(staged: bool) {
    let _scope = crate::drawpipe::test_scope::Scope::new();
    for compress in [false, true] {
        let slow = if compress { (99, 112) } else { (14, 16) };
        for (tdiv, (frames, nominal_frames)) in [(1e-6, (142, 160)), (10e-6, slow)] {
            let (m, mut sw, b, p) = normal_lines_plan(tdiv, compress);
            let id = Identity::default();
            assert_eq!(b.mask, 15);
            assert_eq!(oos_hal::dho::seq::compress_k(&b, &p) > 1, compress && tdiv == 10e-6);
            assert_eq!(b.frames, frames);
            sw.f2_funding = if staged {
                crate::drawpipe::funding::Profile::parse(crate::drawpipe::funding::TEST_PROFILE)
            } else {
                Default::default()
            };
            let pipe = crate::drawpipe::pipe(&sw).unwrap();
            pipe.publish_view(&m, 970, 448, sw.interp, id);
            let (worker, limited) = deposit_plan(&b, &p, id, true);
            assert!(worker.is_some());
            assert_eq!(limited, frames, "compression={compress} tdiv={tdiv}");
            // The reviewed nominal geometry precedes the production screen margin.
            let nominal = Settings { depth: 0, ..b.clone() };
            let np = hz::prep(nominal.mask, oos_hal::dho::seq::tdiv_fs(nominal.tdiv), nominal.depth, 0);
            let nb = super::super::batch_plan(&nominal, &np, sw.board.zone_batch);
            assert_eq!(nb.frames, nominal_frames);
            assert_eq!(deposit_plan(&nb, &np, id, true).1, nominal_frames);
            let (worker, limited) = deposit_plan(&b, &p, id, false);
            assert!(worker.is_none());
            assert_eq!(limited, frames);
            // Standard may be fast eligible, but has no active Batch route or Batch view.
            pipe.set_view(None, 970, 448, sw.interp);
            let (worker, limited) = deposit_plan(&b, &p, id, true);
            assert!(worker.is_none());
            assert_eq!(limited, frames);
        }
    }
}

fn normal_lines_plan(tdiv: f32, compress: bool) -> (Model, crate::sysconf::Switches, Settings, hz::Params) {
    let mut m = Model {
        tscale: crate::model::tscales().iter().position(|&t| (t - tdiv).abs() < 1e-12).unwrap(),
        dots: crate::model::DotsMode::Off,
        ..Default::default()
    };
    for ch in &mut m.ch {
        ch.on = true;
    }
    let sw = crate::sysconf::Switches { compress, ..Default::default() };
    let s = super::super::settings(&m, false, &sw);
    let p = hz::prep(s.mask, oos_hal::dho::seq::tdiv_fs(s.tdiv), s.depth, 0);
    let b = super::super::batch_plan(&s, &p, sw.board.zone_batch);
    (m, sw, b, p)
}

#[test]
fn four_channel_line_batches_keep_every_record_with_matching_and_raw_newest() {
    for compress in [false, true] {
        for tdiv in [1e-6, 10e-6] {
            for match_cores in [false, true] {
                check_lines_readout(tdiv, compress, match_cores);
            }
        }
    }
}

fn check_lines_readout(tdiv: f32, compress: bool, match_cores: bool) {
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let counters = crate::drawpipe::counters();
    let (m, sw, mut b, p) = normal_lines_plan(tdiv, compress);
    b.header = false;
    let (input, count, _) = scripted::draw_route_batch(&m, &sw, 0);
    let fb = input.frame_bytes();
    let raw_newest = super::super::raw_newest(&b, &p).then(|| {
        let (raw, ..) = scripted::draw_route_batch(&m, &Default::default(), 1);
        raw.newest(0)
    });
    let pipe = Pipe::start(1, 0, counters);
    let id = Identity::default();
    pipe.publish_view(&m, 970, 448, sw.interp, id);
    crate::board::dho::publication::read(count as u32, count as u32, count);
    let readout = Readout::new(
        &pipe,
        (&b, &p),
        (fb, count, false),
        (count as u64, HeaderProbe::Off, raw_newest),
        (id, Instant::now()),
    );
    let mut cores = Cores::default();
    let mut dma = input.into_bytes();
    let frames = readout.dispatch(&mut dma, Some(Default::default()), match_cores.then_some(&mut cores), |f| f);
    assert!(frames.is_empty());
    let recycled = crate::drawpipe::host::finish_batch(&pipe, 1).unwrap();
    assert_eq!(recycled.bytes().len(), fb * count);
    let mut picture = Phosphor::new(970, 448);
    let got = pipe.accept(&mut picture, &mut true, &mut [0; 4], &mut (0.0, 0.0));
    assert_eq!(got.frames, count as u64);
    assert!(picture.map.iter().all(|ch| ch.iter().any(|&px| px > 0)));
    let newest = pipe.take_waveform((970, 448)).unwrap();
    assert_eq!(newest.seq, count as u64);
    assert_eq!(newest.dt, 1.0 / p.rate as f64);
    assert!(!newest.envelope);
    assert!(counters.records_text().contains(&format!("read={count} deposited={count}")));
    assert_eq!(counters.read().2, 0);
}

struct Bytes(Vec<u8>);
impl RawBatch for Bytes {
    fn bytes(&self) -> &[u8] {
        &self.0
    }
    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any + Send> {
        self
    }
}

fn record(n: u32, amplitude: i32) -> Vec<u8> {
    (0..n)
        .flat_map(|i| ((32768 + amplitude * [4, -2, 1, -3, -1, 3, -4, 2][i as usize % 8]) as u16).to_le_bytes())
        .collect()
}

fn finish(pipe: &crate::drawpipe::Pipe) {
    crate::drawpipe::host::finish_batch(pipe, 1).expect("the completed batch returns its buffer");
}

fn stale_frame(bytes: &[u8], s: &Settings, p: &hz::Params, warm: bool) -> Frame {
    // An independent oracle for the pattern valid before frame 0: raw if nothing was learned yet.
    let mut corrected = bytes.to_vec();
    if warm {
        for (i, b) in corrected.as_chunks_mut::<2>().0.iter_mut().enumerate() {
            let code = u16::from_le_bytes([b[0], b[1]]) as i32 - 200 * [4, -2, 1, -3, -1, 3, -4, 2][i % 8];
            b.copy_from_slice(&(code as u16).to_le_bytes());
        }
    }
    super::super::decode_probed(&corrected, s, p, 0, HeaderProbe::Off)
}

fn run(frame0_drop: bool, replacement: bool, warm: bool) {
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let counters = crate::drawpipe::counters();
    // AC coupling: frame 1 is not checked for its edge here (frame1.rs has those tests)
    let s = Settings {
        mask: 1,
        header: false,
        compress: 1,
        trig_couple: oos_hal::dho::tpu::Coupling::Ac,
        ..Settings::default()
    };
    let p = hz::Params { memsel: 4096, ..hz::prep(s.mask, 1_000_000_000, 4096, 0) };
    // A stale frame with a fitting but different amplitude would change the learned pattern.
    let (stale, good) = (record(p.memsel, 250), record(p.memsel, 200));
    let (mut cores, mut oracle) = (Cores::default(), Cores::default());
    if warm {
        super::super::decode_with(&good, &s, &p, 0, HeaderProbe::Off, Some(&mut cores));
        super::super::decode_with(&good, &s, &p, 0, HeaderProbe::Off, Some(&mut oracle));
    }
    let mut want = Phosphor::new(128, 64);
    let mut model = Model::default();
    for (i, ch) in model.ch.iter_mut().enumerate() {
        ch.on = i == 0;
    }
    let expected_stale = if frame0_drop || !replacement {
        stale_frame(&stale, &s, &p, warm)
    } else {
        super::super::decode_with(&stale, &s, &p, 0, HeaderProbe::Off, Some(&mut oracle))
    };
    // frame 0 is left out with `frame0_drop` (upstream 0cc8fcb6): the workers start at frame 1
    if replacement && !frame0_drop {
        crate::drawpipe::draw_frame(&mut want, &model, &expected_stale, Interp::Exact, None);
    }
    let neighbour = record(p.memsel, 220);
    if replacement {
        let frame = super::super::decode_with(&neighbour, &s, &p, 2, HeaderProbe::Off, Some(&mut oracle));
        crate::drawpipe::draw_frame(&mut want, &model, &frame, Interp::Exact, None);
    }
    let expected_newest = super::super::decode_with(&good, &s, &p, 3, HeaderProbe::Off, Some(&mut oracle));
    let pipe = Pipe::start(1, 0, counters);
    pipe.publish_view(&model, 128, 64, Interp::Exact, Default::default());
    let frames = Readout::new(
        &pipe,
        (&s, &p),
        (good.len(), 3, frame0_drop),
        (3, HeaderProbe::Off, None),
        (Default::default(), Instant::now()),
    )
    .dispatch(
        &mut Bytes([stale, neighbour, good].concat()),
        replacement.then(|| Bytes(Vec::new())),
        Some(&mut cores),
        |frame| Frame { stamp: 17, cfg: 23, ..frame },
    );
    assert_eq!(cores.pattern((1, 0, p.rate)), oracle.pattern((1, 0, p.rate)), "learning includes an ineligible frame");
    let drawn = if frame0_drop { 1 } else { 2 };
    let newest = newest(&pipe, frames, (replacement, drawn), &want);
    assert_eq!((newest.seq, newest.stamp, newest.cfg), (3, 17, 23));
    assert_eq!(newest.ch, expected_newest.ch);
    accounted(counters, replacement, frame0_drop);
}

fn accounted(counters: &crate::drawpipe::Counters, replacement: bool, frame0_drop: bool) {
    let dropped = counters.read().2;
    let deposited = counters.deposited.load(std::sync::atomic::Ordering::Relaxed);
    let frame0 = u64::from(frame0_drop);
    let buffer = if replacement { 0 } else { 2 - frame0 };
    assert_eq!(deposited + u64::from(!replacement) + frame0 + buffer, 3, "every read record is accounted for");
    assert_eq!(dropped, if replacement { 0 } else { 2 });
    let text = counters.records_text();
    assert!(text.contains(&format!("lost.frame0={frame0}")));
    assert!(text.contains(&format!("lost.buffer={buffer}")));
}

fn newest(pipe: &Pipe, frames: Vec<Frame>, (replacement, drawn): (bool, usize), want: &Phosphor) -> Frame {
    if replacement {
        assert!(frames.is_empty());
        finish(pipe);
        let mut got = Phosphor::new(128, 64);
        let accepted = pipe.accept(&mut got, &mut false, &mut [0; 4], &mut (0.0, 0.0));
        assert_eq!(accepted.frames, drawn as u64 + 1);
        let newest = pipe.take_waveform((128, 64)).unwrap();
        let mut expected = Phosphor::new(128, 64);
        expected.add_maps(&want, [true; 4]);
        let mut model = Model::default();
        for (c, ch) in model.ch.iter_mut().enumerate() {
            ch.on = c == 0;
        }
        crate::drawpipe::draw_frame(&mut expected, &model, &newest, Interp::Exact, None);
        assert_eq!(got.maps(), expected.maps(), "the complete batch includes its corrected newest frame");
        std::sync::Arc::try_unwrap(newest.into_frame()).ok().unwrap()
    } else {
        assert_eq!(frames.len(), 1);
        frames.into_iter().next().unwrap()
    }
}

#[test]
fn acquisition_batch_stale_frame_draws_without_learning() {
    for warm in [false, true] {
        for frame0_drop in [false, true] {
            run(frame0_drop, true, warm);
        }
    }
}

#[test]
fn acquisition_batch_buffer_exhaustion_learns_only_the_newest() {
    for warm in [false, true] {
        for frame0_drop in [false, true] {
            run(frame0_drop, false, warm);
        }
    }
}

#[test]
fn unavailable_deposit_intake_falls_back_to_the_record_queue() {
    use crate::board::dho::publication;

    let _scope = crate::drawpipe::test_scope::Scope::new();
    let counters = crate::drawpipe::counters();
    let pipe = Pipe::start(0, 0, counters);
    let s = Settings { mask: 1, header: false, compress: 1, ..Settings::default() };
    let p = hz::prep(s.mask, 1_000_000_000, 4096, 0);
    let raw = record(p.memsel, 200);
    let mut dma = Bytes(raw.repeat(3));
    let identity = Identity::default();
    publication::read(3, 3, 3);
    let readout =
        Readout::new(&pipe, (&s, &p), (raw.len(), 3, false), (3, HeaderProbe::Off, None), (identity, Instant::now()));
    let frames = readout.dispatch(&mut dma, Some(Bytes(Vec::new())), None, |frame| frame);
    let shared = crate::board::HwShared::default();
    publication::push_frames(&shared, frames, s.mode);
    let shared = oos_hal::sync::lock(&shared);
    assert_eq!(shared.queue.len(), 1);
    assert_eq!((shared.queue[0].seq, shared.frame.seq), (3, 3));
    assert_eq!(counters.read(), (3, 0, 2));
    assert!(counters.records_text().contains("lost.unclaimed=2"));
    assert!(pipe.take_buffer().is_some());
}

fn publish_readout(
    pipe: &Pipe,
    shared: &crate::board::HwShared,
    (settings, geometry): (&Settings, &hz::Params),
    seq: u64,
    replacement: Option<Bytes>,
) {
    use crate::board::dho::publication;

    let id = oos_hal::sync::lock(shared).acquisition.identity();
    let raw = record(geometry.memsel, 200);
    let mut dma = Bytes(raw.repeat(3));
    publication::read(3, 3, 3);
    let reservation = pipe.staged_taps().then(|| pipe.reserve_plan(id).unwrap());
    let readout = Readout::new(
        pipe,
        (settings, geometry),
        (raw.len(), 3, false),
        (seq, HeaderProbe::Off, None),
        (id, Instant::now()),
    )
    .reserved(reservation);
    publication::push_frames(
        shared,
        readout.dispatch(&mut dma, replacement, None, |f| publication::tag(f, id)),
        settings.mode,
    );
}

#[test]
fn replacement_dma_refusal_waits_for_the_older_unpublished_tap() {
    for profile in [crate::drawpipe::funding::TEST_PROFILE, ""] {
        replacement_dma_refusal(profile);
    }
}

fn replacement_dma_refusal(profile: &str) {
    use crate::ui::{host::Host, lifecycle::test_wait::HANG_GUARD, Ui};
    use crate::wave::Source;
    use oos_hal::sync::lock;
    use std::sync::{mpsc, Arc, Mutex};

    let _scope = crate::drawpipe::test_scope::Scope::new();
    let sw = crate::sysconf::Switches::parse(&|_| None, profile, "", Default::default());
    let pipe = crate::drawpipe::pipe(&sw).unwrap();
    let shared = crate::board::HwShared::default();
    let mut ui = Ui::new(Source::Hw { shared: shared.clone(), seen: 0 }, Host::headless());
    let (m, _, mut s, p) = normal_lines_plan(1e-6, false);
    ui.m = m;
    ui.m.draw_batch = true;
    s.header = false;
    ui.step(0.03);
    let id = lock(&shared).acquisition.identity();
    assert!(deposit_plan(&s, &p, id, true).0.is_some());
    let (entered, started) = mpsc::sync_channel(1);
    let (release, resume) = mpsc::channel();
    let gate = Mutex::new(Some((entered, resume)));
    let settings = s.clone();
    let hold = Arc::new(move |bytes: &[u8], frame: &mut Frame| {
        if let Some((entered, resume)) = lock(&gate).take() {
            entered.send(()).unwrap();
            resume.recv().unwrap();
        }
        super::super::decode_into(bytes, &settings, &p, 1, frame);
    });
    let raw = record(p.memsel, 200);
    let fb = raw.len();
    assert!(pipe.feed_with(Box::new(Bytes(raw)), fb, 0..1, id, hold, None, None).is_none());
    started.recv_timeout(HANG_GUARD).unwrap();

    let dispatch = |seq, replacement| publish_readout(pipe, &shared, (&s, &p), seq, replacement);
    dispatch(4, Some(Bytes(Vec::new())));
    dispatch(7, None);
    ui.step(0.03);
    release.send(()).unwrap();
    assert!(matches!(ui.src, Source::Hw { seen: 0, .. }));
    assert_eq!(lock(&shared).queue[0].seq, 7);
    crate::drawpipe::host::finish_batch(pipe, 1).unwrap();
    crate::drawpipe::host::finish_batch(pipe, 4).unwrap();
    ui.step(0.03);
    assert!(matches!(ui.src, Source::Hw { seen: 4, .. }));
    assert_eq!(lock(&shared).frame.seq, 7, "reading the older tap leaves latest publication intact");
    dispatch(10, Some(Bytes(Vec::new())));
    crate::drawpipe::host::finish_batch(pipe, 10).unwrap();
    ui.step(0.03);
    assert!(matches!(ui.src, Source::Hw { seen: 7, .. }));
    ui.step(0.03);
    assert!(matches!(ui.src, Source::Hw { seen: 10, .. }));
    assert!(lock(&shared).queue.is_empty());
    assert_eq!(pipe.tap_count(), 0);
    assert!(crate::drawpipe::counters().records_text().contains("lost.buffer=2 "));
}

#[test]
fn sparse_remap_admission_reduces_the_arm_before_any_read() {
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let (m, sw, b, p) = normal_lines_plan(10e-9, false);
    let id = Identity::default();
    let pipe = crate::drawpipe::pipe(&sw).unwrap();
    pipe.publish_view(&m, 1024, 448, sw.interp, id);
    assert_eq!(b.frames, 250);
    assert_eq!(deposit_plan(&b, &p, id, true).1, 182);
}

#[test]
fn one_raw_dot_record_selects_lossless_wide_admission() {
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let (mut m, sw, _, _) = normal_lines_plan(500e-6, false);
    m.mdepth = crate::model::mdepths().iter().position(|d| d.1 == 1_000_000).unwrap();
    m.dots = crate::model::DotsMode::On;
    let s = super::super::settings(&m, false, &sw);
    let p = hz::prep(s.mask, oos_hal::dho::seq::tdiv_fs(s.tdiv), s.depth, 0);
    assert_eq!((p.memsel, p.rate), (1_000_000, 156_250_000));
    let hits = crate::picture::deposit::standard::dot_hit_bound(271.0 / (p.rate as f64 * s.tdiv * 10.0));
    assert_eq!(hits, 2884);
    assert_eq!(hits * 24, 69_216);
    let pipe = crate::drawpipe::pipe(&sw).unwrap();
    let id = Identity::default();
    pipe.publish_view(&m, 271, 448, sw.interp, id);
    let format = crate::picture::deposit::count::CountFormat::for_hits(hits);
    assert_eq!(format, crate::picture::deposit::count::CountFormat::Wide);
    assert_eq!(pipe.read_limit(id, Some((p.rate, 1))), Some((crate::picture::deposit::HIT_LIMIT / 15) as u32));
    assert!(hits > crate::picture::deposit::HIT_LIMIT);
}

mod tap_contract;

mod admission;
