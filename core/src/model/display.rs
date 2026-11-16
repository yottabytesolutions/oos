// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

/// What the plot area shows: the scope traces, or one of the analysers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AView {
    Scope,
    Spectrum,
    Eye,
    Power,
    Bode,
    /// bandwidth test: step response of the front end (ui/bwtest.rs)
    Step,
    Magnetic,
    Inverter,
    Dpt,
}

/// Power analysis page.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PowerMode {
    Quality,
    Harmonics,
    Switching,
    Ripple,
    /// C = q / v over the turn-on record
    Capacitance,
    /// rail overshoot, undershoot, DC and turn-on maximum
    Rail,
}

/// Waveform histogram: off, vertical (voltage distribution of the whole screen), horizontal (time distribution in a
/// band of +-0.5 div around the trigger level).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Hist {
    Off,
    Vertical,
    Horizontal,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RunState {
    Run,
    Stop,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TrigStatus {
    Triggered,
    Auto,
    Wait,
    Stop,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AcqMode {
    Normal,
    Peak,
    Average,
    HiRes,
}

/// What the QUICK key (and the Quick button of the top bar) does; set in Utility > Quick key.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Quick {
    /// PPM of the screen in /tmp (the behaviour before the setting existed)
    Screenshot,
    /// PNG in the storage directory (Storage > Save screenshot)
    SavePng,
    SaveSetup,
    SaveCsv,
    Autoset,
    Single,
    RunStop,
    Clear,
    /// the next of Display > Views (ui/views.rs)
    NextView,
}

pub const QUICKS: [Quick; 9] = [
    Quick::Screenshot,
    Quick::SavePng,
    Quick::SaveSetup,
    Quick::SaveCsv,
    Quick::Autoset,
    Quick::Single,
    Quick::RunStop,
    Quick::Clear,
    Quick::NextView,
];

impl Quick {
    /// Name in the Utility menu and its picker.
    pub fn name(self) -> &'static str {
        match self {
            Quick::Screenshot => "Screenshot",
            Quick::SavePng => "Save PNG",
            Quick::SaveSetup => "Save setup",
            Quick::SaveCsv => "Save CSV",
            Quick::Autoset => "Autoset",
            Quick::Single => "Single",
            Quick::RunStop => "Run/Stop",
            Quick::Clear => "Clear",
            Quick::NextView => "Next view",
        }
    }

    /// Label of the Quick button in the top bar.
    pub fn short(self) -> &'static str {
        match self {
            Quick::Screenshot => "Snap",
            Quick::SavePng => "PNG",
            Quick::SaveSetup => "Setup",
            Quick::SaveCsv => "CSV",
            Quick::Autoset => "Auto",
            Quick::Single => "Single",
            Quick::RunStop => "Run/Stop",
            Quick::Clear => "Clear",
            Quick::NextView => "View",
        }
    }
}

/// What an HDMI monitor shows. Mirror: the panel picture, scaled. Waveform: a settings line and the waveform area,
/// scaled. Layout: the whole screen laid out again at the monitor's size, sharp, with the traces stretched; it costs a
/// second render per monitor frame (about 5 to 10 ms on the unit at 1920 x 1080), so the panel may get fewer frames.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum HdmiMode {
    Mirror,
    #[default]
    Waveform,
    Layout,
}

pub const HDMI_MODES: [HdmiMode; 3] = [HdmiMode::Mirror, HdmiMode::Waveform, HdmiMode::Layout];

/// Display > Dots (docs/fork/display.md): Off draws lines between the points, On a dot per point, Auto chooses per
/// picture (`trace_style`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum DotsMode {
    Off,
    On,
    #[default]
    Auto,
}

pub const DOTS_MODES: [DotsMode; 3] = [DotsMode::Off, DotsMode::On, DotsMode::Auto];

/// How a trace is drawn.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TraceStyle {
    Vectors,
    Dots,
    /// lines, and the real samples marked over them (`Ui::draw_sample_marks`)
    VectorsMarked,
}

/// Auto marks the samples once they lie this many pixels apart on the screen (the interpolated, sparse regime). At 4 px
/// (20 ns/div, unit 2026-11-14) the marks made a dense dotted band over the trace.
pub const MARK_SPACING_PX: f64 = 8.0;

/// The trace style for `mode`: in XY (`xy`), or for samples `x_per_sample` pixels apart on the screen. Auto: dots in
/// XY, lines with the samples marked from `MARK_SPACING_PX` apart, else lines. Off: lines; On: dots.
pub fn trace_style(mode: DotsMode, xy: bool, x_per_sample: f64) -> TraceStyle {
    match mode {
        DotsMode::Off => TraceStyle::Vectors,
        DotsMode::On => TraceStyle::Dots,
        DotsMode::Auto if xy => TraceStyle::Dots,
        DotsMode::Auto if x_per_sample >= MARK_SPACING_PX => TraceStyle::VectorsMarked,
        DotsMode::Auto => TraceStyle::Vectors,
    }
}

/// Size of the text on the screen (Display > Text size): Normal is the original look, Large and Larger use fonts 2 and
/// 4 px bigger with the rows, cards and dialogs grown to match. A preference: Default keeps it.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum TextSize {
    #[default]
    Normal,
    Large,
    Larger,
}

pub const TEXT_SIZES: [TextSize; 3] = [TextSize::Normal, TextSize::Large, TextSize::Larger];

impl TextSize {
    pub fn name(self) -> &'static str {
        match self {
            TextSize::Normal => "Normal",
            TextSize::Large => "Large",
            TextSize::Larger => "Larger",
        }
    }

    /// 0, 1, 2: the font set and the layout metrics.
    pub fn index(self) -> usize {
        self as usize
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CursorMode {
    Off,
    Time,
    Volt,
    /// time cursors that read the active channel where they stand
    Track,
    /// time and voltage cursors at once
    Xy,
}

/// How the plot is split into waveform windows; every channel and math channel stands in one of them.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum WinLayout {
    #[default]
    One,
    /// two side by side
    Cols,
    /// two one above the other
    Rows,
    /// two by two
    Quad,
}

impl WinLayout {
    pub const ALL: [WinLayout; 4] = [WinLayout::One, WinLayout::Cols, WinLayout::Rows, WinLayout::Quad];

    pub fn count(self) -> usize {
        match self {
            WinLayout::One => 1,
            WinLayout::Cols | WinLayout::Rows => 2,
            WinLayout::Quad => 4,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            WinLayout::One => "One",
            WinLayout::Cols => "Side by side",
            WinLayout::Rows => "Stacked",
            WinLayout::Quad => "Four",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Grid {
    Full,
    Light,
    None,
}

/// How much of the 1024×600 picture the plot takes. The same four apply to the panel and to a scaled window of it,
/// and to the waveform on the external display.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Density {
    /// top bar, plot, measurement strip, cards
    #[default]
    Scope,
    /// plot and measurement strip
    Full,
    /// plot, and under it a pane of numbers or two stacked traces
    Split,
    /// four equal panes
    Four,
}

impl Density {
    pub const ALL: [Density; 4] = [Density::Scope, Density::Full, Density::Split, Density::Four];

    pub fn label(self) -> &'static str {
        match self {
            Density::Scope => "Scope",
            Density::Full => "Full",
            Density::Split => "Split",
            Density::Four => "Four",
        }
    }
}

/// Where the measurement strip is drawn. Under is below the graticule.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StripDock {
    Under,
    Left,
    Right,
    Above,
}

impl StripDock {
    pub const ALL: [StripDock; 4] = [StripDock::Under, StripDock::Left, StripDock::Right, StripDock::Above];

    pub fn label(self) -> &'static str {
        match self {
            StripDock::Under => "Under",
            StripDock::Left => "Left",
            StripDock::Right => "Right",
            StripDock::Above => "Above",
        }
    }
}
