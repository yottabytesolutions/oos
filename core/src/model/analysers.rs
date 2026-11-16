// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! Settings types of the eye placement and the magnetic, inverter and double-pulse analysers.

/// Where the eye is drawn. The full page is the default. Side and Under are extra panes and leave that page available.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EyePlace {
    /// The eye fills the plot: the figures, the jitter column and the charts.
    Full,
    /// A pane to the right of the trace.
    Side,
    /// A pane under the trace.
    Under,
}

/// Magnetic plot: current against flux linkage, or B against H when the core is set.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MagPlot {
    IPsi,
    BH,
}

/// Inverter page.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InvPage {
    Dq0,
    Mech,
}

/// Where the inverter takes its angle.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InvAngle {
    /// θ from the phase trio (DQ0 only)
    Off,
    Phase,
    Analog,
    Quad,
    Hall,
}

/// The optional fourth probe of a double pulse.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DptFourth {
    Off,
    Gate,
    DiodeV,
    DiodeI,
}

/// Which edge the double-pulse plot is zoomed to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DptZoom {
    Both,
    TurnOn,
    TurnOff,
}
