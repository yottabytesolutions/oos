// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

pub(crate) fn run() {
    let p = oos_hal::model::platform();
    println!("model     {}", p.ident());
    println!("caps      {}", p.caps_line());
    println!("series    {}, bandwidth code {}, EDU {}", p.caps.series, p.caps.bw_code, p.caps.edu);
    println!("mac       {}", p.mac.as_deref().unwrap_or("-"));
    let l = &p.limits;
    println!("srate     {:?} Sa/s (1 / 2 / 3-4 channels)", l.max_srate);
    println!("mem       {:?} pts, LA on {:?}", l.max_mem, l.max_mem_la);
    println!(
        "afg max   sine {} square {} ramp {} pulse {} noise {} Hz",
        l.afg_max_hz.sine, l.afg_max_hz.square, l.afg_max_hz.ramp, l.afg_max_hz.pulse, l.afg_max_hz.noise
    );
    println!("clocks    ADC {} Hz, generator {} Hz", l.adc_clock_hz, l.afg_clock_hz);
    for u in &p.unit {
        println!("unit      {} ({})", u, oos_hal::model::UNIT_CONF);
    }
    for o in &p.overrides {
        println!("EXPERIMENTAL  {}", o);
    }
    for w in &p.warnings {
        println!("warning   {}", w);
    }
}
