// SPDX-License-Identifier: GPL-2.0-or-later

use super::super::dump::{BatchInput, Dump};
use super::*;
use crate::board::dho::cores::Cores;
use crate::drawpipe::stage::fixture_bench::{verify_fixture, Stage};
use crate::model::{tscales, DotsMode, Meas, Model, WaveId};
use crate::sparse::Interp;
use oos_hal::dho::seq;
use std::fs;

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "oos-batch-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        Self(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn synthetic(scratch: &Scratch, zoom: bool, header: bool, points: i128) -> Fixture {
    let mut model = Model {
        tscale: tscales().iter().position(|&t| (t - 1e-6).abs() < 1e-12).unwrap(),
        dots: DotsMode::Off,
        zoom,
        delay: 1e-8,
        trig_level: if zoom || header { -0.05 } else { 3.0 },
        ..Model::default()
    };
    model.ch_label[0] = "omitted label".into();
    model.ch[0].vfine = 1.4;
    if points > 64 * 1024 {
        model.meas.push((Meas::Vmax, WaveId::Ch(0)));
    }
    let s = seq::Settings {
        mask: 1,
        tdiv: 1e-6,
        delay: f64::from(model.delay),
        header,
        window: None,
        ..seq::Settings::default()
    };
    let mut p = oos_hal::dho::hz::prep(s.mask, seq::tdiv_fs(s.tdiv), points, 0);
    if !zoom && !header {
        // A decimated record with no crossing exercises the wide-search counter.
        p.rate /= 2;
        p.dot_fs *= 2;
    }
    let fb = seq::frame_bytes(&s, &p);
    let mut bytes = Vec::new();
    for n in 0..3u16 {
        if header {
            for word in [0xfa05, 0, 0x7b, 0, n, 1, 2, n] {
                bytes.extend(word.to_le_bytes());
            }
        }
        for i in 0..p.memsel {
            let swing: i32 = if i < p.pre { -3800 } else { 3800 };
            let code = 32768 + swing + [36, -9, 13, -27, -40, 5, 7, 15][(i + u32::from(n)) as usize % 8];
            bytes.extend((code as u16).to_le_bytes());
        }
    }
    assert_eq!(bytes.len(), 3 * fb);
    let bytes: Arc<[u8]> = bytes.into();
    let mut cores = Cores::default();
    let seed = cores.fixture_seed(1024, |i| 32000 + (i % 8) as u16 * 7).unwrap();
    cores.restore_fixture_pattern((s.mask, 0, p.rate), seed);
    let mut dump = Dump::open(scratch.0.clone(), 2, "synthetic".into()).unwrap();
    let pending = dump.capture(((0, 0), 1), make_input_with(&s, &p, &cores), &bytes, (None, None)).unwrap();
    let mut dma = Some(Bytes(bytes.clone()));
    publish(&model, (&s, &p), (&mut dma, &mut cores), (fb, pending.trace()));
    assert!(dump.capture(((0, 0), 1), make_input_with(&s, &p, &cores), &bytes, (None, None)).is_some());
    assert!(dump.capture(((0, 0), 1), make_input_with(&s, &p, &cores), &bytes, (None, None)).is_none());
    assert!(dump.capture(((1, 0), 1), make_input_with(&s, &p, &cores), &bytes, (None, None)).is_some());
    let path = scratch.0.join("batch-000");
    assert_eq!(fs::read(path.join("dma.bin")).unwrap(), bytes.as_ref());
    assert!(!fs::read_to_string(path.join("fixture.txt")).unwrap().contains("omitted label"));
    Fixture::load(&path).unwrap()
}

fn make_input_with<'a>(s: &'a seq::Settings, p: &'a oos_hal::dho::hz::Params, cores: &'a Cores) -> BatchInput<'a> {
    BatchInput::new((s, p), (3, 3, false), (cores, true))
}

#[test]
fn v2_fixture_ignores_retired_play_metadata_and_checks_the_dma_layout() {
    let scratch = Scratch::new();
    let expected = synthetic(&scratch, false, false, 7032);
    let path = scratch.0.join("batch-000");
    let sidecar = path.join("fixture.txt");
    let meta = fs::read_to_string(&sidecar).unwrap();
    assert_eq!(meta.lines().next(), Some("oos-batch.v2"));
    for key in ["play=", "play_inferred=", "play_window=", "play_linear_step=", "play_frame_len="] {
        assert!(meta.lines().all(|line| !line.starts_with(key)));
    }
    let legacy = format!(
        "{meta}play=stock\nplay_inferred=true\nplay_window=6250 391\nplay_linear_step=3834792227\nplay_frame_len=7000\nfuture_layout=extra\n"
    );
    fs::write(&sidecar, &legacy).unwrap();
    let loaded = Fixture::load(&path).unwrap();
    assert_eq!(loaded.settings(), expected.settings());
    assert!(loaded.records().eq(expected.records()));

    let raw_bytes = expected.records().next().unwrap().len();
    let interpolated_bytes = 2 * 7000;
    assert_ne!(raw_bytes, interpolated_bytes);
    let legacy = legacy
        .replace(&format!("frame_bytes={raw_bytes}\n"), &format!("frame_bytes={interpolated_bytes}\n"))
        .replace(&format!("frame_stride={raw_bytes}\n"), &format!("frame_stride={interpolated_bytes}\n"));
    fs::write(&sidecar, legacy).unwrap();
    fs::write(path.join("dma.bin"), vec![0; 3 * interpolated_bytes]).unwrap();
    assert_eq!(Fixture::load(&path).err().as_deref(), Some("invalid DMA size, frame count or stride"));
}

fn publish(
    model: &Model,
    (s, p): (&seq::Settings, &oos_hal::dho::hz::Params),
    (dma, cores): (&mut Option<Bytes>, &mut Cores),
    (fb, trace): (usize, Box<RouteTrace>),
) {
    let counters = Box::leak(Box::default());
    let workers = if p.memsel > 64 * 1024 { 4 } else { 2 };
    let pipe = Pipe::start(workers, 0, counters);
    let id = Identity::default();
    let zoom = model.zoom.then(|| crate::drawpipe::route::ZoomView::new((170, 95), 1.5, 0.05));
    pipe.publish_picture_view(model, (300, 180), Interp::Exact, id, zoom);
    let frames = Readout::new(&pipe, (s, p), (fb, 3, false), (3, HeaderProbe::Off, None), (id, Instant::now()))
        .trace(Some(trace))
        .dispatch(dma, Some(cores), |f| f, &|| false);
    assert!(frames.is_empty());
    assert!(drawpipe::host::finish_batch(&pipe, 0).unwrap().into_any().downcast::<Bytes>().is_ok());
    assert_eq!(drawpipe::shutdown::close(&pipe, Instant::now() + HANG_GUARD), 0);
    assert!(counters.records_text().contains("deposited=3"), "{}", counters.records_text());
}

#[test]
fn fixture_dump_replays_dense_header_and_zoom_routes() {
    for (zoom, header, route) in [(false, false, "dense"), (false, true, "dense"), (true, false, "zoom")] {
        let scratch = Scratch::new();
        let fixture = synthetic(&scratch, zoom, header, 16_384);
        let observed = fixture.expected_route().unwrap().0;
        assert!(observed.contains(&format!(" {route}=3 ")), "expected {route}: {observed}");
        assert_eq!(fixture.patterns()[0].unwrap().parts().1, 1024.0);
        assert!(replay_route(&fixture));
        if !header && route == "dense" {
            assert!(observed.ends_with(" wide_search=3"), "{observed}");
            let stages = [
                Stage::Geometry,
                Stage::Trigger,
                Stage::ColumnEdges,
                Stage::MinMax,
                Stage::CodeToRow,
                Stage::Writes,
                Stage::Reset,
                Stage::Place,
            ];
            verify_fixture(&fixture, &stages.map(|stage| (stage, 3)));
            verify_fixture(&fixture, &[(Stage::Core, 0), (Stage::Decode, 0)]);
        }
        if header {
            verify_fixture(&fixture, &[(Stage::Decode, 0), (Stage::MinMax, 3), (Stage::CodeToRow, 3)]);
        }
        let original = scratch.0.join("batch-000/fixture.txt");
        let text = fs::read_to_string(&original).unwrap();
        fs::write(&original, text.replace("frames=3\n", "frames=4\n")).unwrap();
        assert!(Fixture::load(original.parent().unwrap()).is_err());
    }
}

#[test]
fn replay_captured_batches() {
    for_each_fixture(&mut |fixture| {
        replay_route(fixture);
    });
}

#[test]
#[ignore = "requires captured fixtures in OOS_FIXTURES"]
fn captured_batches_reuse_row_and_grid_caches() {
    let pipe = Pipe::start(1, 0, Box::leak(Box::default()));
    let mut before = (0, 0);
    let mut batches = 0;
    for_each_fixture(&mut |fixture| {
        let view = fixture.view().unwrap();
        let (model, size, interp, zoom) = view.inputs();
        pipe.publish_picture_view(model, size, interp, Identity::default(), zoom);
        let (send, receive) = std::sync::mpsc::channel();
        let mut trace = RouteTrace::new(move |report| {
            let counts = crate::fastdraw::with_scratch(|scratch| scratch.envelope.cache_rebuilds());
            send.send((report, counts)).unwrap();
        });
        trace.set_replay_prior_hits(view.scheduling().1);
        dispatch_fixture(fixture, &pipe, Some(trace));
        drawpipe::host::finish_batch(&pipe, batches as u64).unwrap();
        let (report, after) = receive.recv().unwrap();
        let (route, _) = fixture.expected_route().unwrap();
        fixture.assert_route(report.route(), &report.range());
        println!("{}: row builds={}, grid builds={}", fixture.name(), after.0 - before.0, after.1 - before.1);
        if batches == 0 && !route.contains(" dense=0 ") {
            assert!(after.0 > 0 && after.1 > 0, "dense placement exercises both caches");
        } else if batches != 0 {
            assert_eq!(after, before, "{}: warm batch", fixture.name());
        }
        before = after;
        batches += 1;
    });
    drawpipe::shutdown::close(&pipe, Instant::now() + HANG_GUARD);
    assert!(batches > 1, "use a group of consecutive batches");
}

#[test]
fn fixture_benchmark_uses_the_recorded_split_workers() {
    let scratch = Scratch::new();
    let fixture = synthetic(&scratch, false, false, 131_072);
    verify_fixture(&fixture, &[(Stage::Split, 3), (Stage::MinMax, 3), (Stage::CodeToRow, 3), (Stage::Fold, 2)]);
}

#[test]
fn fixture_round_trips_the_armed_settings_and_raw_newest() {
    let scratch = Scratch::new();
    let mut s = seq::Settings {
        mask: 1,
        trigger: seq::Trig::Pulse {
            pol: oos_hal::dho::tpu::Polarity::Neg,
            when: oos_hal::dho::tpu::When::In,
            lower: 1e-9,
            upper: 3e-9,
        },
        holdoff: 2e-8,
        noise_reject: true,
        ..Default::default()
    };
    s.ch[0].invert = true;
    s.ch[0].couple = seq::Couple::Ac;
    s.ch[0].bw = 20;
    let p = oos_hal::dho::hz::prep(s.mask, seq::tdiv_fs(s.tdiv), 4096, 0);
    let fb = seq::frame_bytes(&s, &p);
    let frame = crate::board::Frame {
        ch: [vec![1.5, 5.0], Vec::new(), Vec::new(), Vec::new()],
        dt: 4e-9,
        pre: 1,
        acq: (2.5e8, 2),
        scan: true,
        roll: true,
        partial: true,
        fixed_ti: Some(0.25),
        ..Default::default()
    };
    let cores = Cores::default();
    let input = BatchInput::new((&s, &p), (3, 2, false), (&cores, false));
    let mut dump = Dump::open(scratch.0.clone(), 1, "synthetic".into()).unwrap();
    let words = [0x8001, 0x4213, 0xa1bf];
    let pending = dump.capture(((0, 0), 1), input, &vec![0; 3 * fb], (Some(&frame), Some(&words))).unwrap();
    pending.finish_records();

    let path = scratch.0.join("batch-000");
    let fixture = Fixture::load(&path).unwrap();
    let observed = fixture.settings().0;
    assert_eq!((observed.holdoff, observed.noise_reject), (s.holdoff, s.noise_reject));
    assert_eq!(observed.trigger, s.trigger);
    assert_eq!(observed.ch[0], s.ch[0]);
    let (_, (_, avail, _), newest) = fixture.source();
    assert_eq!(avail, 2);
    let newest = newest.unwrap();
    assert_eq!(newest.ch, frame.ch);
    assert!(newest.roll && newest.scan && newest.partial);
    assert_eq!(newest.fixed_ti, frame.fixed_ti);
    let la = fixture.la().unwrap();
    assert_eq!(la.words.as_slice(), words);
    let sidecar = fs::read_to_string(path.join("fixture.txt")).unwrap();
    assert!(sidecar.lines().last().unwrap().starts_with("dump_io_ns="));
}
