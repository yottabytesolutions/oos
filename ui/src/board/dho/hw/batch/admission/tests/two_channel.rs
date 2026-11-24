// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;
use crate::board::dho::switches::HeaderProbe;
use crate::drawpipe::RawBatch;
use crate::sparse::Interp;
use crate::wave::Phosphor;

fn actual() -> (Pipe, Settings, hz::Params, Identity, Model) {
    let (pipe, _, _, id, mut model) = fixture();
    for (key, value) in [
        ("ch3.on", "false"),
        ("ch4.on", "false"),
        ("trig_src", "1"),
        ("trig_level", "4.93"),
        ("ch1.vdiv", "0.01"),
        ("ch1.offset", "-1.5"),
        ("ch2.vdiv", "0.2"),
        ("ch2.vfine", "1.125"),
        ("ch2.offset", "-4.9"),
    ] {
        crate::store::set(&mut model, key, value).unwrap();
    }
    let switches = crate::sysconf::Switches::default();
    let original = crate::board::dho::hw::settings(&model, false, &switches);
    let params = hz::prep(original.mask, oos_hal::dho::seq::tdiv_fs(original.tdiv), original.depth, 0);
    let settings = crate::board::dho::hw::batch_plan(&original, &params, switches.board.zone_batch);
    assert_eq!((settings.mask, params.rate, params.memsel), (3, 625_000_000, 7032));
    assert_eq!((settings.header, settings.compress, settings.trig_src), (false, 1, 2));
    assert_eq!(oos_hal::dho::seq::frame_bytes(&settings, &params), 28128);
    pipe.publish_view(&model, 1024, 480, Interp::Exact, id);
    (pipe, settings, params, id, model)
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

#[test]
fn bounded_two_channel_readout_keeps_all_selected_pixels_and_original_newest() {
    for prefix in [false, true] {
        for matched in [false, true] {
            check_pixels(prefix, matched);
        }
    }
}
fn check_pixels(prefix: bool, matched: bool) {
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let (_, settings, params, id, model) = actual();
    let pipe = Pipe::start(1, 0, crate::drawpipe::counters());
    pipe.publish_view(&model, 1024, 480, Interp::Exact, id);
    // Pixel comparison keeps the same 64-record workload under the new source grant.
    let count = 64 + usize::from(prefix);
    let fb = oos_hal::dho::seq::frame_bytes(&settings, &params);
    let one: Vec<u8> = (0..params.memsel)
        .flat_map(|i| {
            let ch1 = 32768u16 + 1000 * (i % 3) as u16;
            let ch2 = if i < params.pre { 22768u16 } else { 42768u16 };
            [ch1.to_le_bytes(), ch2.to_le_bytes()].concat()
        })
        .collect();
    assert_eq!(one.len(), fb);
    let mut expected = Phosphor::new(1024, 480);
    let mut oracle = Cores::default();
    for index in usize::from(prefix)..count {
        let frame = crate::board::dho::hw::decode_with(
            &one,
            &settings,
            &params,
            index as u64 + 1,
            HeaderProbe::Off,
            matched.then_some(&mut oracle),
        );
        crate::drawpipe::draw_frame(&mut expected, &model, &frame, Interp::Exact, None);
    }
    let mut raw = Some(Bytes(one.repeat(count)));
    crate::board::dho::publication::read(crate::drawpipe::counters(), count as u32, count as u32, count);
    let mut cores = Cores::default();
    let frames = super::super::super::Readout::new(
        &pipe,
        (&settings, &params),
        (fb, count, prefix),
        (count as u64, HeaderProbe::Off, None),
        (id, Instant::now()),
    )
    .dispatch(&mut raw, matched.then_some(&mut cores), |f| f, &|| false);
    assert!(frames.is_empty());
    assert!(raw.is_none());
    let recycled = crate::drawpipe::host::finish_batch(&pipe, 0).unwrap();
    assert_eq!(recycled.bytes().len(), fb * count);
    let mut got = Phosphor::new(1024, 480);
    assert_eq!(pipe.accept(&mut got, &mut true, &mut [0; 4], &mut (0.0, 0.0)).frames, 64);
    let mut normalized = Phosphor::new(1024, 480);
    normalized.add_maps(&expected, [true, true, false, false]);
    assert_eq!(got.maps(), normalized.maps(), "all64 selected records preserve exact pixels");
    let newest = pipe.take_waveform((1024, 480)).unwrap();
    assert_eq!(newest.seq, count as u64, "prefix exclusion never renumbers physical newest");
    let text = crate::drawpipe::counters().records_text();
    assert!(text.contains(&format!("read={count} deposited=64 shown=0")), "{text}");
    assert!(text.contains(&format!("lost.frame0={}", usize::from(prefix))));
    assert!(text.contains("lost.frame1=0"));
}
