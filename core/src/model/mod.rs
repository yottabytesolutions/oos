// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! Flat transitional model. Group ownership and change classification arrive in later M2 parts.

mod acquisition;
mod channel;
mod decoder;
mod defaults;
mod display;
mod generator;
mod settings;
mod signal;
mod tables;
mod trigger;

use crate::config::Config;
use crate::math::{MathOp, Window};
use crate::protocols::Context;
pub use crate::protocols::{Proto, BUILTIN_PROTOS};
pub use channel::*;
pub use decoder::*;
pub use display::*;
pub use generator::*;
pub use tables::*;
pub use trigger::*;

pub const NCH: usize = oos_dsp::CHANNELS;
pub const NGEN: usize = 1;
pub const HDIV: f32 = 10.0;
pub const VDIV: f32 = 8.0;
pub const VIEWS_MAX: usize = 4;

#[derive(Clone, PartialEq, Debug)]
pub struct Model {
    pub ch: [Channel; NCH],
    pub active: usize, // channel the vertical knobs act on
    pub tscale: usize,
    pub delay: f32, // s, horizontal position of the trigger point relative to the centre
    pub trig_src: usize,
    /// edge and pulse trigger on a logic analyser line instead of `trig_src` (`trig_line`)
    pub trig_la: Option<u8>,
    /// pattern trigger: D0..D15 (H / L / X), with the LA on
    pub pat_la: [Bit; 16],
    pub trig_level: f32,
    pub slope: Slope,
    pub trig_mode: TrigMode,
    /// trigger holdoff, seconds (0 = the shortest there is)
    pub holdoff: f32,
    pub trig_type: TrigType,
    /// second level: low threshold (slope, runt, window), level of B (delay), clock level (setup/hold)
    pub trig_level2: f32,
    /// noise reject: wider comparator hysteresis (0.7 instead of 0.3 div)
    pub trig_nrej: bool,
    /// the auto set-up sets noise reject from the noise on the trigger signal's flat levels (opt-in, off by default)
    pub auto_nrej: bool,
    /// coupling of the edge trigger on a channel (`trig_coupling` gives the one in effect)
    pub trig_coup: TrigCoup,
    /// second source: B (delay), clock (setup/hold)
    pub trig_src2: usize,
    pub trig_slope2: Slope,
    pub trig_pol: Pol,
    pub trig_when: WhenKind,
    pub trig_t1: f32,
    pub trig_t2: f32,
    pub video_std: VideoStd,
    pub video_sync: VSync,
    pub video_line: u32,
    pub pat: [Bit; NCH],
    pub pat_levels: [f32; NCH],
    pub win_mode: WinMode,
    pub nth_idle: f32,
    pub nth_n: u32,
    pub setup: f32,
    pub hold: f32,
    pub run: RunState,
    pub status: TrigStatus,
    pub acq: AcqMode,
    /// the offsets of the interleaved ADC cores taken off in software (`board::dho::cores`; Acquire > ADC cores)
    pub cores: bool,
    pub avg_count: u32,
    /// Average mode: the FPGA averages (docs/fork/specs/01-hw-averaging.md) where the board can (`Board::avg_hw`), else
    /// the software average; experimental, not verified on the unit
    pub avg_hw: bool,
    /// segmented recording: records per RUN/SINGLE (0 = off), browsed with the history afterwards
    pub segments: usize,
    /// segments recorded by the FPGA (one arm, hardware re-arm and trigger stamps) instead of the software history;
    /// experimental, not verified on the unit
    pub seg_hw: bool,
    /// hardware segments: wait after each segment before the next arm, seconds (0 = back to back)
    pub seg_ivl: f32,
    pub mdepth: usize,  // index into `config.depths()` (0 = auto)
    pub persist: usize, // index into PERSIST
    pub intensity: u32, // 10..100 %
    pub grid: Grid,
    /// graticule brightness in % (10..100), and whether the frame with its ticks is drawn
    pub grid_bright: u32,
    pub grid_frame: bool,
    /// waveform windows, and the window (0-based) of each channel and math channel
    pub win_layout: WinLayout,
    pub ch_win: [usize; NCH],
    pub math_win: [usize; crate::math::NMATH],
    /// window of the LA lanes while the plot is split (one window: their strip under the plot)
    pub la_win: usize,
    pub meas: Vec<(Meas, usize)>,
    /// delay and phase: entry of `ui::tools::PAIRS`, channel A, channel B
    pub meas2: Vec<(usize, usize, usize)>,
    pub dvm_on: bool,
    pub dvm_src: usize,
    pub dvm_mode: usize,
    pub cnt_on: bool,
    pub cnt_src: usize,
    pub cnt_mode: usize,
    pub cnt_digits: u32,
    /// user names of the channels ("" = CHn)
    pub ch_label: [String; NCH],
    pub cursor: CursorMode,
    pub cur_a: f32, // fraction of the screen 0..1 (time: x, volt: y)
    pub cur_b: f32,
    /// voltage cursors of the XY mode
    pub cur_c: f32,
    pub cur_d: f32,
    pub touch_lock: bool,
    pub aux_out: AuxOut,
    /// fine V/div: the scale knob walks the rows of the calibration table (`Board::vscale_steps_nv`), SCPI
    /// `:CHANnel<n>:SCALe` takes any value; one switch for all channels
    pub fine_v: bool,
    /// fine time base on the knob
    pub fine_t: bool,
    /// fine time base: factor on the step of `tscale`, 1.0 = on the step; the acquisition keeps the step
    pub tfine: f32,
    /// the scale changes around ground, not the middle of the screen
    pub v_expand_gnd: bool,
    pub h_expand: HExpand,
    pub math: MathOp,
    pub math_a: usize,
    pub math_b: usize,
    pub math_scale: usize, // index into math::math_scales() (waveform) or FFT_DB (FFT)
    /// corner frequency of the math low-/high-pass (Hz)
    pub math_fc: f32,
    /// Linear: A x + B
    pub math_lin: (f32, f32),
    /// the formula of each math channel
    pub math_expr: [String; crate::math::NMATH],
    /// data logger interval, index into `LOG_STEPS`
    pub log_ivl: usize,
    /// spectrum limit from a trace: dB above it
    pub spec_margin: f32,
    pub math_offset: f32, // divisions
    pub fft_win: Window,
    /// bandwidth of the math band-pass/band-stop (Hz)
    pub math_bw: f32,
    /// the math channel the fields above edit (0..NMATH); the others are kept in `maths`
    pub math_sel: usize,
    pub maths: [crate::math::MathCh; crate::math::NMATH],
    /// the other decoders; None = not touched yet; the slot of `dec_sel` is not used
    pub decs: [Option<DecCfg>; DECODERS],
    pub dec_sel: usize,
    /// second pane under the plot, index into `ui::tables::PANES`
    pub pane: usize,
    /// what the measurements measure, drawn on the traces (measlabels.rs)
    pub meas_labels: bool,
    /// what the two charts of the Trend pane show: 0 = by the list, 1 = off, k = row k - 2 of the measurement list
    pub trend_sel: [usize; 2],
    pub dec_on: bool,
    pub dec_src: usize,
    pub dec_baud: usize, // index into decode::BAUDS
    /// CAN bit rate, index into decode::CAN_RATES
    pub dec_canrate: usize,
    /// CAN probed on CAN_H (dominant high) instead of the RX/TX logic level
    pub dec_canh: bool,
    /// CAN-FD data phase bit rate (index into decode::FD_RATES)
    pub dec_fdrate: usize,
    /// ARINC 429 bit rate (index into decode::ARINC_RATES, 0 = auto)
    pub dec_arate: usize,
    /// SENT clock tick (index into decode::SENT_TICKS) and data nibbles per frame (1..6)
    pub dec_tick: usize,
    pub dec_nibbles: u32,
    /// USB speed (index into decode::USB_SPEEDS, 0 = auto)
    pub dec_uspeed: usize,
    /// fourth decoder source (JTAG TDO, Quad-SPI IO1)
    pub dec_src4: Option<usize>,
    /// decoder input: logic analyser lines instead of the channels (`dec_on_la`)
    pub dec_la_in: bool,
    /// LA line per decoder role (source, second, third, fourth; `Proto::roles`), None = not connected
    pub dec_la: [Option<u8>; 4],
    /// RS-485: decode the difference dec_src - dec_diff
    pub dec_diff: Option<usize>,
    /// protocol trigger condition and value
    pub trig_bus: BusWhat,
    /// the value (or lower limit) of the condition: up to 40 bits (I2C, 5 bytes)
    pub trig_bus_val: u64,
    pub ser: SerialTrig,
    pub dec_thr: f32,
    pub dec_ascii: bool,
    pub dec_proto: Proto,
    /// second source: SCL (I2C), CLK (SPI)
    pub dec_src2: usize,
    /// SPI chip select (active low), None = frames from clock pauses
    pub dec_cs: Option<usize>,
    pub dec_mode: u8,
    /// data bits: UART 5..9, SPI 4..32
    pub dec_bits: u32,
    pub dec_parity: oos_dsp::decode::Parity,
    /// option settings of a protocol module (`protocols::Opt`), indices into its choices
    pub dec_opt: [u8; crate::protocols::OPTS],
    pub zoom: bool,
    /// slow time bases on the hardware: roll from 50 ms/div (on), scan from 200 ms/div (off), like stock "Auto Roll"
    pub auto_roll: bool,
    pub zscale: usize, // index into tscales(), <= tscale
    pub zdelay: f32,   // centre of the zoom window relative to the trigger
    /// colour-graded persistence: trace pixels coloured by hit density (temperature map)
    pub color_grade: bool,
    /// Display > Dots: lines, dots, or Auto (`trace_style`)
    pub dots: DotsMode,
    /// what an HDMI monitor shows (Display > HDMI)
    pub hdmi: HdmiMode,
    /// Display > Views: how many screen arrangements there are to switch between (1: off; ui/views.rs)
    pub views: usize,
    /// the one shown (0-based)
    pub view_cur: usize,
    /// what each view keeps while another one is shown (ui/views.rs, `Ui::view_snapshot`); empty: not set up yet
    pub view_slots: [String; VIEWS_MAX],
    /// Z input: this channel sets the brightness of the other traces (low edge of the screen = dark)
    pub z_src: Option<usize>,
    /// histogram of the active channel from the phosphor map
    pub hist: Hist,
    pub view: AView,
    /// spectrum analyser: source, centre/span (Hz, span 0 = 0 .. fs/2), RBW (0 = auto), window, trace mode
    /// (0 normal, 1 average, 2 max hold, 3 min hold), reference level (dBV, top of the grid), waterfall, distortion
    pub spec_src: usize,
    pub spec_center: f64,
    pub spec_span: f64,
    pub spec_rbw: f64,
    pub spec_win: Window,
    /// peak table order: 0 = by frequency, 1 = by level
    pub spec_sort: usize,
    /// span of the pane, index into `ui::PANE_HARMS`
    pub spec_harm: usize,
    pub spec_trace: usize,
    pub spec_ref: f32,
    pub spec_wf: bool,
    pub spec_dist: bool,
    /// eye diagram: source, bit rate (0 = recovered from the edges)
    pub eye_src: usize,
    pub eye_rate: f64,
    /// power analysis: voltage and current source, page, line frequency (0 = from the voltage)
    pub pw_v: usize,
    pub pw_i: usize,
    pub pw_mode: PowerMode,
    /// Bode plot: sweep start/stop (Hz), points, generator amplitude (Vpp), input/output channel, generator preset
    /// and LAN address
    pub bode_start: f64,
    pub bode_stop: f64,
    pub bode_points: usize,
    pub bode_amp: f64,
    pub bode_in: usize,
    pub bode_out: usize,
    pub bode_gen: usize,
    pub bode_addr: String,
    /// the external generator is on the USB-A port (USBTMC, the first one found) instead of `bode_addr`
    pub bode_usb: bool,
    /// Bode with a generator that sweeps on its own (`ui::bode::FREE`, same start/stop): sweep time (s), logarithmic
    /// (else linear), sync channel, its edge and level (V), input channel (0 = none, k = CHk: then the gain is against
    /// `bode_ref` dBV)
    pub bode_sw_time: f64,
    pub bode_sw_log: bool,
    pub bode_sync: usize,
    pub bode_sync_slope: Slope,
    pub bode_sync_level: f32,
    pub bode_sw_in: usize,
    pub bode_ref: f64,
    /// hardware source: bumped with settings a measurement must wait for (`board::Shared::stamp`); not stored
    pub hw_stamp: u64,
    /// hardware source: bumped when the hardware average has to start over (CLEAR, RUN, SINGLE); not stored
    pub hw_avg_clear: u64,
    /// AUTO sweep with the shortest auto-trigger time, set by the auto set-up while it reads levels; not stored
    pub auto_fast: bool,
    /// built-in generator: output on, frequency (Hz), amplitude (Vpp), offset (V); sine
    /// bandwidth test (Analysers > Bandwidth): source, signal frequency hint (0 = Auto), bins per period
    pub bw_src: usize,
    pub bw_hint: f64,
    pub bw_bins: usize,
    /// which generator the `afg_*` fields below are, the settings of the others
    pub gen_sel: usize,
    pub gen_bank: [GenCh; NGEN],
    pub afg_on: bool,
    /// arbitrary wave: id of `Board::arb_store` (0 = none yet), channel it is captured from
    pub afg_arb: u32,
    pub afg_arb_src: usize,
    pub afg_freq: f64,
    pub afg_ampl: f64,
    pub afg_offset: f64,
    /// index into AFG_WAVES; duty (square, pulse) and symmetry (ramp) in %
    pub afg_wave: usize,
    pub afg_duty: f64,
    pub afg_symmetry: f64,
    /// modulation: index into AFG_MODS, rate in Hz, AM depth in %, FM deviation in Hz, PM deviation in degrees,
    /// shape index into AFG_MOD_SHAPES
    pub afg_mod: usize,
    pub afg_mod_rate: f64,
    pub afg_mod_depth: f64,
    pub afg_fm_dev: f64,
    pub afg_pm_dev: f64,
    pub afg_mod_shape: usize,
    /// mask test: on/off, source channel, tolerances (divisions) used when the mask is created, stop on fail
    pub mask_on: bool,
    /// zone trigger (zone.rs): two zones in waveform coordinates
    pub zones: [crate::zone::Zone; 2],
    pub mask_src: usize,
    pub mask_x: f32,
    pub mask_y: f32,
    pub mask_stop: bool,
    /// action of the QUICK key
    pub quick: Quick,
    /// size of the text on the screen
    pub text_size: TextSize,
    /// logic analyser D0..D15 (DHO900, ui/la.rs)
    pub la: crate::la::LaSet,
}
