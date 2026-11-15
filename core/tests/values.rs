// SPDX-License-Identifier: GPL-2.0-or-later

use oos_core::change::normalize;
use oos_core::config::Config;
use oos_core::model::*;
use oos_core::protocols::{Context, ModuleInfo, ModuleValues, Opt};

fn config(channels: usize, generators: usize, la: bool) -> Config {
    Config::new(
        channels,
        generators,
        la,
        [true, true, true, false],
        &[("Auto", 0), ("100M", 100_000_000)],
        [1.25e9, 625e6, 312.5e6],
        [[125_000_000, 62_500_000, 31_250_000], [31_250_000, 12_500_000, 10_000_000]],
    )
}

#[test]
fn capabilities_are_independent_values_in_one_process() {
    let (two, four) = (config(2, 0, false), config(4, 1, true));
    let mut full = Model::default();
    full.ch[3].on = true;
    full.active = 3;
    full.la.on = true;
    full.math_a = 3;
    full.maths[1].a = NCH;
    full.dec_src4 = Some(3);
    full.zones[0].src = 3;
    let mut small = full.clone();
    assert!(!normalize(&mut full, &four));
    assert!(normalize(&mut small, &two));
    assert_eq!((small.ch[3].on, small.active, small.la.on), (false, 0, false));
    assert_eq!((small.math_a, small.maths[1].a, small.dec_src4, small.zones[0].src), (0, NCH, None, 0));
    assert_eq!((small.bode_sync, small.bode_sw_in), (1, 1));
    assert!(!normalize(&mut small, &two));
    assert!(full.la_on_in(&four));
    assert!(!full.la_on_in(&two));
    assert_eq!((two.generators(), four.generators()), (0, 1));
    assert_eq!((two.srate(0), two.srate(2), two.srate(4)), (1.25e9, 625e6, 312.5e6));
    assert_eq!((two.mem(0, false), four.mem(4, true)), (125_000_000, 10_000_000));
    assert!(two.source_exists(NCH, NCH) && !two.source_exists(3, NCH));
}

#[test]
fn injected_metadata_and_evaluated_settings_drive_module_queries() {
    let info = [ModuleInfo {
        key: "Fixture",
        name: "Fixture protocol",
        roles: [Some("Data"), Some("Clock"), None, None],
        diff_ok: true,
        options: &[Opt { name: "Rate", choices: &["Slow", "Fast"], default: 0 }],
    }];
    let ctx = Context::new(&info, Some(ModuleValues { id: 0, bit_time: 2e-6, frame_span: Some(80e-6) }));
    let other = Context::new(&info, Some(ModuleValues { id: 0, bit_time: 4e-6, frame_span: None }));
    let mut m = Model {
        dec_proto: Proto::Module(0),
        dec_src: 0,
        dec_src2: 2,
        dec_opt: [255; 4],
        trig_type: TrigType::Bus,
        ..Model::default()
    };
    assert_eq!(Proto::from_key_in("Fixture", &ctx), Some(m.dec_proto));
    assert_eq!(m.dec_proto.name_in(&ctx), "Fixture protocol");
    assert_eq!(m.dec_proto.roles_in(&ctx)[1], Some("Clock"));
    assert!(m.dec_proto.diff_ok_in(&ctx));
    assert_eq!(m.dec_opts_in(&ctx), [1, 0, 0, 0]);
    assert_eq!(m.bus_bit_in(&ctx), 2e-6);
    assert_eq!(m.trig_feature_in(&ctx), 2e-6 / 20.0);
    assert_eq!(m.trig_span_in(&ctx), 80e-6);
    assert_eq!(m.trig_span_in(&other), 200.0 * 4e-6);
    assert_eq!(m.trig_channels_in(&config(4, 1, true), &ctx), [true, false, true, false]);
    m.dec_proto = Proto::Uart;
    assert_eq!(m.dec_opts_in(&ctx), [0; 4]);
    assert_eq!(m.trig_span_in(&ctx), 200.0 / oos_core::decode::BAUDS[m.dec_baud]);
}

#[test]
fn depth_and_la_values_preserve_the_existing_cap_rules() {
    let config = config(4, 1, true);
    let ctx = Context::new(&[], None);
    let mut m = Model { mdepth: 1, trig_src: 1, ..Model::default() };
    assert_eq!(m.depth_cap_in(&config, &ctx), Some((62_500_000, Some(1))));
    assert_eq!(m.depth_label_in(&config, &ctx), "100M → 62500000 (trig CH2)");
    let rate = m.max_srate_in(&config);
    m.la.on = true;
    assert_eq!(m.max_srate_in(&config), rate / 2.0);
    assert_eq!(m.max_mem_in(&config), 31_250_000);
    m.ch[0].coupling = Coupling::Dc50;
    assert!(normalize(&mut m, &config));
    assert_eq!(m.ch[0].coupling, Coupling::Dc);
}
