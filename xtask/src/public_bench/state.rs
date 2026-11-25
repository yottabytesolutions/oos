// SPDX-License-Identifier: GPL-2.0-or-later

use super::{
    report::{Metric, Values},
    Clock, Error,
};
use crate::unit_tools::scpi::{Scpi, STATE};
use std::time::Duration;

#[derive(Debug)]
pub(super) struct State {
    frames: Option<u64>,
    acquired: Option<u64>,
    used: Option<u64>,
    dropped: Option<u64>,
    fps: Option<f64>,
    step_ms: Option<f64>,
    render_ms: Option<f64>,
    present_ms: Option<f64>,
    pub(super) card_mode: Option<String>,
}

impl State {
    pub(super) fn parse(reply: &str) -> Result<Self, Error> {
        if !matches!(
            reply.split_ascii_whitespace().next(),
            Some("fps" | "frames" | "records" | "run" | "status" | "t" | "step" | "render" | "present")
        ) {
            return Err(Error::InvalidReply(STATE));
        }
        // The hardware trailer has a different frames/dropped pair. The UI timing summary
        // precedes edges and parts, which are followed by other stage timing diagnostics.
        let (summary, _) = reply.split_once(" hw frames ").unwrap_or((reply, ""));
        let fields: Vec<_> = summary.split_ascii_whitespace().collect();
        let header_end = fields.iter().position(|field| matches!(*field, "edges" | "parts")).unwrap_or(fields.len());
        let header = &fields[..header_end];
        Ok(Self {
            frames: field(header, "frames").and_then(|v| v.parse().ok()),
            acquired: field(&fields, "acquired").and_then(|v| v.parse().ok()),
            used: field(&fields, "used").and_then(|v| v.parse().ok()),
            dropped: field(&fields, "dropped").and_then(|v| v.parse().ok()),
            fps: field(header, "fps").and_then(number),
            step_ms: timing(header, "step"),
            render_ms: timing(header, "render"),
            present_ms: timing(header, "present"),
            card_mode: fields
                .iter()
                .find_map(|field| field.strip_prefix("sd.mode="))
                .filter(|mode| {
                    matches!(*mode, "legacy" | "hs" | "sdr12" | "sdr25" | "sdr50" | "ddr50" | "sdr104" | "unknown")
                })
                .map(str::to_owned),
        })
    }

    pub(super) fn values_since(&self, start: &Self, duration: Duration) -> Values {
        let rate = |before: Option<u64>, after: Option<u64>| {
            Some(after?.checked_sub(before?)? as f64 / duration.as_secs_f64())
        };
        [
            (Metric::PicturesPerS, rate(start.frames, self.frames)),
            (Metric::AcquiredPerS, rate(start.acquired, self.acquired)),
            (Metric::ShownPerS, rate(start.used, self.used)),
            (Metric::DroppedPerS, rate(start.dropped, self.dropped)),
            (Metric::ReportedFps, self.fps),
            (Metric::StepMs, self.step_ms),
            (Metric::RenderMs, self.render_ms),
            (Metric::PresentMs, self.present_ms),
        ]
        .into()
    }
}

fn field<'a>(fields: &[&'a str], name: &str) -> Option<&'a str> {
    fields.windows(2).find(|entry| entry[0] == name).map(|entry| entry[1])
}

fn number(value: &str) -> Option<f64> {
    value.parse::<f64>().ok().filter(|n| n.is_finite() && *n >= 0.0)
}

fn timing(fields: &[&str], name: &str) -> Option<f64> {
    let entry = fields.windows(3).find(|entry| entry[0] == name && entry[2] == "ms")?;
    number(entry[1])
}

pub(super) fn read_state(scope: &mut impl Scpi, clock: &impl Clock) -> Result<(Duration, State), Error> {
    let reply = scope.query_timed(STATE, || clock.now()).map_err(Error::Scpi)?;
    let stamp = reply.before_write + (reply.after_line - reply.before_write) / 2;
    Ok((stamp, State::parse(&reply.line)?))
}
