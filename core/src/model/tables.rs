// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

/// V/div at the probe tip for 1x. Below 1 mV/div the AFE stays at its highest analog gain and the FPGA read path
/// scales digitally (cal_vertical digital gain 2.15 / 5.38 / 10.76 at 500 / 200 / 100 uV, no forced 20 MHz).
pub const VSCALES: [f32; 16] =
    [100e-6, 200e-6, 500e-6, 1e-3, 2e-3, 5e-3, 10e-3, 20e-3, 50e-3, 100e-3, 200e-3, 500e-3, 1.0, 2.0, 5.0, 10.0];
/// Fastest time/div the ADC acquisition uses. Stored refs (SIM) may display faster than this.
pub const HW_MIN_TDIV: f32 = 1e-9;

/// s/div: 50 ps .. 1000 s. Below 1 ns/div is a display scale for stored records; the live ADC still runs at
/// `HW_MIN_TDIV` (`hw_time_per_div`).
pub fn tscales() -> Vec<f32> {
    TIME_SCALES.to_vec()
}

/// The same 1-2-5 ladder, available to allocation-free domain stages, built as `tscales` builds it upstream.
pub const TIME_SCALES: [f32; 41] = time_scales();
const fn time_scales() -> [f32; 41] {
    let mut values = [0.0; 41];
    let mut d = 1e-11_f64;
    let (mut i, mut k) = (0, 0);
    while i < values.len() {
        let value = d * [1.0, 2.0, 5.0][k % 3];
        if value >= 0.49e-10 {
            values[i] = value as f32;
            i += 1;
        }
        k += 1;
        if k % 3 == 0 {
            d *= 10.0;
        }
    }
    values
}

/// FFT vertical scale in dB/div.
pub const FFT_DB: [f32; 5] = [1.0, 2.0, 5.0, 10.0, 20.0];

/// Bandwidth limits of a channel (AFE low-pass): Auto = filter ~ fs / 4 as the
/// default, Full = widest filter (stock "OFF", 3150 MHz in the AFE), GND = no signal through the AFE; then the AFE
/// filter frequency in MHz (`board::dho::hw::bw_code`).
pub const BWS: [(&str, u32); 31] = [
    ("Auto", 0),
    ("Full", 0),
    ("20 MHz", 20),
    ("GND", 0),
    ("5 MHz", 5),
    ("10 MHz", 10),
    ("15 MHz", 15),
    ("25 MHz", 25),
    ("30 MHz", 30),
    ("35 MHz", 35),
    ("40 MHz", 40),
    ("50 MHz", 50),
    ("60 MHz", 60),
    ("70 MHz", 70),
    ("75 MHz", 75),
    ("80 MHz", 80),
    ("90 MHz", 90),
    ("100 MHz", 100),
    ("125 MHz", 125),
    ("150 MHz", 150),
    ("175 MHz", 175),
    ("200 MHz", 200),
    ("250 MHz", 250),
    ("300 MHz", 300),
    ("310 MHz", 310),
    ("400 MHz", 400),
    ("600 MHz", 600),
    ("800 MHz", 800),
    ("1 GHz", 1000),
    ("1.5 GHz", 1500),
    ("2 GHz", 2000),
];
/// Index of "20 MHz" in BWS (old setups, SCPI 20M).
pub const BW_20M: usize = 2;
/// Index of "Full" in BWS.
pub const BW_FULL: usize = 1;
/// Index of "Auto" in BWS: the filter follows the sample rate (the default, as on the stock system; with Full the
/// noise at slow time bases can exceed the trigger hysteresis, trigger.md 7).
pub const BW_AUTO: usize = 0;

/// Wave shapes of the built-in generator (`Board::gen`).
pub const AFG_WAVES: [&str; 7] = ["Sine", "Square", "Ramp", "Pulse", "Noise", "DC", "Arb"];
/// Indices into AFG_WAVES the generator page treats apart.
pub const AFG_NOISE: usize = 4;
pub const AFG_DC: usize = 5;
/// Modulations and modulating shapes of the built-in generator (`Board::gen`).
pub const AFG_MODS: [&str; 4] = ["Off", "AM", "FM", "PM"];
pub const AFG_MOD_SHAPES: [&str; 6] = ["Sine", "Square", "Triangle", "Up ramp", "Down ramp", "Noise"];

/// Persistence: name, seconds (0 = minimum, <0 = infinite).
pub const PERSIST: [(&str, f32); 7] =
    [("Min", 0.0), ("100 ms", 0.1), ("500 ms", 0.5), ("1 s", 1.0), ("5 s", 5.0), ("10 s", 10.0), ("Infinite", -1.0)];

/// Engineering format: 3 significant digits by default, SI prefix, unit.
pub fn si(v: f64, unit: &str) -> String {
    si_digits(v, unit, 3)
}

pub fn si_digits(v: f64, unit: &str, digits: usize) -> String {
    if !v.is_finite() {
        return format!("---- {}", unit);
    }
    if v == 0.0 {
        return format!("0 {}", unit);
    }
    let prefixes = [
        (1e12, "T"),
        (1e9, "G"),
        (1e6, "M"),
        (1e3, "k"),
        (1.0, ""),
        (1e-3, "m"),
        (1e-6, "\u{b5}"),
        (1e-9, "n"),
        (1e-12, "p"),
    ];
    let a = v.abs();
    let (mult, p) = prefixes.iter().copied().find(|(m, _)| a >= *m * 0.9995).unwrap_or((1e-12, "p"));
    let x = v / mult;
    let ax = x.abs();
    let dec = if ax >= 100.0 {
        digits.saturating_sub(3)
    } else if ax >= 10.0 {
        digits.saturating_sub(2)
    } else {
        digits.saturating_sub(1)
    };
    format!("{:.*} {}{}", dec, x, p, unit)
}

/// Compact scale label: "200 µs", "1 V", "500 mV" (no trailing zeros).
pub fn si_short(v: f64, unit: &str) -> String {
    let s = si_digits(v, unit, 3);
    // "1.00 V" -> "1 V", "2.50 V" -> "2.5 V"
    if let Some((num, rest)) = s.split_once(' ') {
        if num.contains('.') {
            let t = num.trim_end_matches('0').trim_end_matches('.');
            return format!("{} {}", t, rest);
        }
    }
    s
}
