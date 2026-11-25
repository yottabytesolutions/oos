// SPDX-License-Identifier: GPL-2.0-or-later

use super::Error;
use crate::unit_tools::scpi::Scpi;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const ERROR_QUERY: &str = ":SYST:ERR?";
const STRIP: &str = ":MEAS:STRIP?";

#[derive(Clone, Copy, Debug)]
pub(super) struct View {
    pub(super) name: &'static str,
    channels: u8,
    scale: &'static str,
    feature: Feature,
}

#[derive(Clone, Copy, Debug)]
enum Feature {
    Normal,
    Peak,
    HiRes,
    Average,
    Zoom,
    Measurements,
}

impl View {
    const fn normal(name: &'static str, channels: u8, scale: &'static str) -> Self {
        Self { name, channels, scale, feature: Feature::Normal }
    }

    const fn feature(name: &'static str, feature: Feature) -> Self {
        Self { name, channels: 4, scale: "1e-6", feature }
    }
}

pub(super) const VIEWS: [View; 18] = [
    View::normal("1ch 5ns", 1, "5e-9"),
    View::normal("1ch 50ns", 1, "5e-8"),
    View::normal("1ch 200ns", 1, "2e-7"),
    View::normal("1ch 1us", 1, "1e-6"),
    View::normal("1ch 10us", 1, "1e-5"),
    View::normal("1ch 100us", 1, "1e-4"),
    View::normal("1ch 1ms", 1, "1e-3"),
    View::normal("2ch 1us", 2, "1e-6"),
    View::normal("4ch 5ns", 4, "5e-9"),
    View::normal("4ch 200ns", 4, "2e-7"),
    View::normal("4ch 1us", 4, "1e-6"),
    View::normal("4ch 10us", 4, "1e-5"),
    View::feature("4ch 1us peak", Feature::Peak),
    View::feature("4ch 1us hires", Feature::HiRes),
    View::feature("4ch 1us average", Feature::Average),
    View::feature("4ch 1us zoom", Feature::Zoom),
    View::feature("4ch 1us measure", Feature::Measurements),
    View::normal("4ch 1us again", 4, "1e-6"),
];

pub(super) type Settings = BTreeMap<String, Readback>;

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub(super) struct Readback {
    requested: Option<String>,
    observed: Option<String>,
    status: Status,
    setter_errors: Vec<i32>,
    query_error: Option<i32>,
}

#[derive(Debug, PartialEq, Deserialize, Serialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum Status {
    Verified,
    Observed,
    Rejected,
    Unavailable,
    Mismatch,
}

#[derive(Debug)]
enum Value<'a> {
    Token(&'a str),
    Number(&'a str),
    AutoDepth,
    EffectiveDepth,
    Strip(&'a [&'a str]),
}

impl Value<'_> {
    fn requested(&self) -> Option<String> {
        match self {
            Self::Token(value) | Self::Number(value) => Some((*value).into()),
            Self::AutoDepth => Some("AUTO".into()),
            Self::EffectiveDepth => None,
            Self::Strip(rows) => Some(rows.join(";")),
        }
    }

    fn observed(&self, reply: &str) -> Option<String> {
        match self {
            Self::Token(_) => token(reply),
            Self::AutoDepth => match reply.to_ascii_uppercase().as_str() {
                "0" | "AUTO" => Some("AUTO".into()),
                _ => token(reply),
            },
            Self::Number(_) | Self::EffectiveDepth => Some(finite(reply)?.to_string()),
            Self::Strip(_) => strip_rows(reply),
        }
    }

    fn matches(&self, observed: &str) -> bool {
        match self {
            Self::Token(value) => token(value).as_deref() == Some(observed),
            Self::Number(value) => finite(value)
                .zip(finite(observed))
                .is_some_and(|(want, actual)| (want - actual).abs() <= (want.abs() * 1e-6).max(1e-15)),
            Self::AutoDepth => observed == "AUTO",
            Self::EffectiveDepth => true,
            Self::Strip(rows) => rows.join(";") == observed,
        }
    }
}

fn token(reply: &str) -> Option<String> {
    let token = reply.trim().to_ascii_uppercase();
    if token.is_empty() || !token.bytes().all(|b| b.is_ascii_alphanumeric()) {
        return None;
    }
    Some(match token.as_str() {
        "0" => "OFF".into(),
        "1" => "ON".into(),
        "NORMAL" => "NORM".into(),
        "POSITIVE" => "POS".into(),
        "AVERAGE" | "AVERAGES" => "AVER".into(),
        "HRESOLUTION" | "HIRES" => "HRES".into(),
        other if other.starts_with("CHANNEL") => other.replacen("CHANNEL", "CHAN", 1),
        _ => token,
    })
}

fn finite(reply: &str) -> Option<f64> {
    let value = reply.trim().parse::<f64>().ok().filter(|number| number.is_finite())?;
    Some(if value == 0.0 { 0.0 } else { value })
}

fn strip_rows(reply: &str) -> Option<String> {
    let mut rows = Vec::new();
    for row in reply.split(';').filter(|row| !row.trim().is_empty()) {
        let (row, number) = row.rsplit_once(',')?;
        finite(number)?;
        let mut fields = row.split(',').map(token);
        let item = match fields.next()??.as_str() {
            "FREQ" => "FREQUENCY".into(),
            "RTIM" => "RTIME".into(),
            item => item.to_owned(),
        };
        let row = std::iter::once(Some(item)).chain(fields).collect::<Option<Vec<_>>>()?;
        rows.push(row.join(","));
    }
    rows.sort_unstable();
    Some(rows.join(";"))
}

fn error_code(reply: &str) -> Result<i32, Error> {
    reply.split_once(',').and_then(|(code, _)| code.trim().parse().ok()).ok_or(Error::InvalidReply(ERROR_QUERY))
}

fn write(scope: &mut impl Scpi, setter: &str) -> Result<Option<i32>, Error> {
    scope.send(setter).map_err(Error::Scpi)?;
    let code = error_code(&scope.query(ERROR_QUERY).map_err(Error::Scpi)?)?;
    Ok((code != 0).then_some(code))
}

fn read(scope: &mut impl Scpi, query: &str, value: Value<'_>, setter_errors: Vec<i32>) -> Result<Readback, Error> {
    // Keep the error query in the same program so even an older build that omits
    // the failed query's response slot returns a complete line.
    let reply = scope.query(&format!("{query};{ERROR_QUERY}")).map_err(Error::Scpi)?;
    let (observation, error) = reply.rsplit_once(';').map_or((None, reply.as_str()), |(v, e)| (Some(v), e));
    let code = error_code(error)?;
    let observed = observation.filter(|_| code == 0).and_then(|reply| value.observed(reply.trim()));
    let status = match observed.as_deref() {
        _ if !setter_errors.is_empty() => Status::Rejected,
        None => Status::Unavailable,
        Some(_) if matches!(value, Value::EffectiveDepth) => Status::Observed,
        Some(actual) if value.matches(actual) => Status::Verified,
        Some(_) => Status::Mismatch,
    };
    Ok(Readback {
        requested: value.requested(),
        observed,
        status,
        setter_errors,
        query_error: (code != 0).then_some(code),
    })
}

fn setting(
    scope: &mut impl Scpi,
    settings: &mut Settings,
    header: &str,
    arg: &str,
    value: Value<'_>,
) -> Result<(), Error> {
    let errors = write(scope, &format!("{header} {arg}"))?.into_iter().collect();
    let readback = read(scope, &format!("{header}?"), value, errors)?;
    settings.insert(header.into(), readback);
    Ok(())
}

pub(super) fn apply(scope: &mut impl Scpi, view: &View, signal: u8) -> Result<Settings, Error> {
    let mut settings = Settings::new();
    let source = format!("CHAN{signal}");
    configure_trigger(scope, &mut settings, &source)?;
    let others = (1..=4).filter(|&channel| channel != signal);
    for (index, channel) in std::iter::once(signal).chain(others).enumerate() {
        let displayed = if index < usize::from(view.channels) { "ON" } else { "OFF" };
        setting(scope, &mut settings, &format!(":CHAN{channel}:DISP"), displayed, Value::Token(displayed))?;
    }
    configure_acquisition(scope, &mut settings, view)?;
    configure_measurements(scope, &mut settings, view, &source)?;
    let errors: Vec<i32> = write(scope, ":RUN")?.into_iter().collect();
    // TRIG:STAT? uses different active statuses. The state header has the Run/Stop field.
    let reply = scope.query(crate::unit_tools::scpi::STATE).map_err(Error::Scpi)?;
    let observed = reply
        .split_ascii_whitespace()
        .zip(reply.split_ascii_whitespace().skip(1))
        .find_map(|(key, value)| (key == "run").then_some(value))
        .filter(|value| matches!(*value, "Run" | "Stop"));
    let status = match observed {
        _ if !errors.is_empty() => Status::Rejected,
        Some("Run") => Status::Verified,
        Some(_) => Status::Mismatch,
        None => Status::Unavailable,
    };
    settings.insert(
        ":RUN".into(),
        Readback {
            requested: Some("Run".into()),
            observed: observed.map(str::to_owned),
            status,
            setter_errors: errors,
            query_error: None,
        },
    );
    Ok(settings)
}

fn configure_trigger(scope: &mut impl Scpi, settings: &mut Settings, source: &str) -> Result<(), Error> {
    for (header, value) in [
        (":TRIG:MODE", "EDGE"),
        (":TRIG:SWE", "NORM"),
        (":TRIG:EDGE:SOUR", source),
        (":TRIG:EDGE:SLOP", "POS"),
        (":TRIG:COUP", "DC"),
        (":TRIG:NREJ", "OFF"),
    ] {
        setting(scope, settings, header, value, Value::Token(value))?;
    }
    for header in [":TRIG:EDGE:LEV", ":TRIG:HOLD", ":TIM:MAIN:OFFS"] {
        setting(scope, settings, header, "0", Value::Number("0"))?;
    }
    Ok(())
}

fn configure_acquisition(scope: &mut impl Scpi, settings: &mut Settings, view: &View) -> Result<(), Error> {
    let feature = view.feature;
    let acquisition = match feature {
        Feature::Peak => "PEAK",
        Feature::HiRes => "HRES",
        Feature::Average => "AVER",
        _ => "NORM",
    };
    setting(scope, settings, ":ACQ:TYPE", acquisition, Value::Token(acquisition))?;
    if matches!(feature, Feature::Average) {
        setting(scope, settings, ":ACQ:AVER", "16", Value::Number("16"))?;
    }
    let errors = write(scope, ":ACQ:MDEP AUTO")?.into_iter().collect();
    settings.insert(":ACQ:MDEP".into(), read(scope, ":SYST:GET? mdepth", Value::AutoDepth, errors)?);
    setting(scope, settings, ":ACQ:MDEP:AUTO", "EXT", Value::Token("EXT"))?;
    setting(scope, settings, ":TIM:MAIN:SCAL", view.scale, Value::Number(view.scale))?;
    let zoom = if matches!(feature, Feature::Zoom) { "ON" } else { "OFF" };
    setting(scope, settings, ":TIM:DEL:ENAB", zoom, Value::Token(zoom))?;
    if zoom == "ON" {
        setting(scope, settings, ":TIM:DEL:SCAL", "1e-7", Value::Number("1e-7"))?;
        setting(scope, settings, ":TIM:DEL:OFFS", "0", Value::Number("0"))?;
    }
    Ok(())
}

pub(super) fn read_effective_depth(scope: &mut impl Scpi, settings: &mut Settings) -> Result<(), Error> {
    settings.insert(":ACQ:MDEP?".into(), read(scope, ":ACQ:MDEP?", Value::EffectiveDepth, Vec::new())?);
    Ok(())
}

fn configure_measurements(
    scope: &mut impl Scpi,
    settings: &mut Settings,
    view: &View,
    source: &str,
) -> Result<(), Error> {
    for header in [":DVM:ENAB", ":COUN:ENAB", ":MEAS:STAT:DISP"] {
        setting(scope, settings, header, "OFF", Value::Token("OFF"))?;
    }
    let errors = write(scope, ":MEAS:CLE ALL")?.into_iter().collect();
    settings.insert(":MEAS:CLE".into(), read(scope, STRIP, Value::Strip(&[]), errors)?);
    if matches!(view.feature, Feature::Measurements) {
        // The strip reports canonical names and we sort rows to compare their membership.
        let items = ["FREQUENCY", "RTIME", "VPP"].map(|item| format!("{item},{source}"));
        let mut errors = Vec::new();
        for item in &items {
            errors.extend(write(scope, &format!(":MEAS:ITEM {item}"))?);
        }
        settings.insert(
            ":MEAS:ITEM".into(),
            read(scope, STRIP, Value::Strip(&items.each_ref().map(String::as_str)), errors)?,
        );
    }
    Ok(())
}
