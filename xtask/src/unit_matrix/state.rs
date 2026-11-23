// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::BTreeMap;

const SECTIONS: &[&str] = &["stage.v1", "work.v1", "route.v1", "source.v1", "present.v1", "fallback.v1"];
const RECORD_KEYS: [&str; 3] = ["records.read", "records.deposited", "records.shown"];
pub(super) const SUMMARY_KEYS: &[&str] = &[
    "eligible",
    "fresh",
    "empty",
    "first_records",
    "direct_records",
    "missing",
    "records.read",
    "records.shown",
    "rejected.age",
    "rejected.age_before_work",
    "lost.queue_overflow",
    "step.missed",
];

pub(super) type Counters = BTreeMap<String, i128>;
pub(super) type Sections = BTreeMap<String, BTreeMap<String, String>>;

#[derive(Debug, Default)]
pub(super) struct State {
    pub(super) healthy: bool,
    pub(super) counters: Counters,
    pub(super) fallback: Counters,
    pub(super) sections: Sections,
}

impl State {
    pub(super) fn parse(reply: &str) -> Self {
        let mut state = Self::default();
        let mut section = None;
        let mut fields = reply.split_ascii_whitespace().peekable();
        while let Some(token) = fields.next() {
            if token == "watchdog" {
                state.healthy = fields.next_if_eq(&"ok").is_some();
            } else if token == "records" {
                for (key, wire) in RECORD_KEYS.into_iter().zip(["read", "deposited", "shown"]) {
                    let Some(value) = fields
                        .next_if(|field| field.starts_with(wire) && field.as_bytes().get(wire.len()) == Some(&b'='))
                    else {
                        break;
                    };
                    if let Ok(count) = value[wire.len() + 1..].parse::<u64>() {
                        state.counters.insert(key.into(), i128::from(count));
                    }
                }
            } else if is_section(token) {
                section = SECTIONS.contains(&token).then_some(token);
                if let Some(name) = section {
                    state.sections.entry(name.into()).or_default();
                }
            } else if let Some((key, value)) = token.split_once('=').filter(|(key, _)| is_key(key)) {
                state.add_field(section, key, value);
            }
        }
        state
    }

    fn add_field(&mut self, section: Option<&str>, key: &str, value: &str) {
        if let Ok(count) = value.parse::<u64>() {
            if section == Some("fallback.v1") {
                self.fallback.insert(key.into(), i128::from(count));
            } else if cumulative_counter(section, key) {
                self.counters.insert(key.into(), i128::from(count));
            }
        }
        if section == Some("source.v1") && key == "wait_admission" {
            if let Some(count) = value.split_once(',').and_then(|(count, _)| count.parse::<u64>().ok()) {
                self.counters.insert("source.wait_admission".into(), i128::from(count));
            }
        }
        if value.split(['/', ',']).all(|part| part.parse::<f64>().is_ok_and(f64::is_finite)) {
            if let Some(fields) = section.and_then(|name| self.sections.get_mut(name)) {
                fields.insert(key.into(), value.into());
            }
        }
    }
}

fn cumulative_counter(section: Option<&str>, key: &str) -> bool {
    // Integer gauges and rolling statistics can fall during normal acquisition.
    SUMMARY_KEYS.contains(&key)
        || key.starts_with("lost.")
        || key.starts_with("rejected.")
        || key.starts_with("records.")
        || key.starts_with("acq_")
        || key.starts_with("stage_restart_")
        || section == Some("route.v1")
        || matches!(
            key,
            "intake.unexpected_full"
                | "admission.unexpected_busy"
                | "pacing.flip_fallback"
                | "stale_worker_push"
                | "stale_ui_publish"
                | "stale_queue_pop"
                | "stale_pipe"
                | "queue_overflow"
                | "fold_drops"
                | "stage.truncated"
                | "flip.shown"
                | "flip.late"
                | "flip.timeout"
                | "flip.modeset"
                | "flip.failed"
                | "helper.skipped"
                | "captured_discarded"
                | "uncorrected"
        )
}

fn is_section(token: &str) -> bool {
    token.split_once(".v").is_some_and(|(name, version)| {
        !name.is_empty()
            && name.bytes().all(|b| b.is_ascii_lowercase())
            && !version.is_empty()
            && version.bytes().all(|b| b.is_ascii_digit())
    })
}

fn is_key(key: &str) -> bool {
    key.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
        && key.bytes().all(|b| b.is_ascii_alphanumeric() || b"_.".contains(&b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_archived_live_state_and_all_six_sections() {
        let state = State::parse(include_str!("fixtures/state.txt"));
        assert!(state.healthy);
        for (key, value) in [
            ("records.read", 499731),
            ("records.deposited", 492983),
            ("records.shown", 492795),
            ("first_records", 492795),
            ("fresh", 5635),
            ("flip.late", 335),
            ("acq_batch_prearm", 6588),
            ("source.wait_admission", 17),
            ("step.missed", 149),
        ] {
            assert_eq!(state.counters[key], value, "{key}");
        }
        for (section, key, value) in [
            ("stage.v1", "read_submit", "0/0/4000"),
            ("work.v1", "refine", "19144/22878"),
            ("present.v1", "flip.late", "335"),
            ("route.v1", "wide_search", "519"),
            ("source.v1", "arm", "0.039/0.074"),
            ("fallback.v1", "decoded_lines", "0"),
        ] {
            assert_eq!(state.sections[section][key], value);
        }
    }

    #[test]
    fn ignores_malformed_fields_and_stops_sections_at_unknown_versions() {
        let state = State::parse("watchdog okay work.v1 signed=-2 quantile=1/2.5 unknown.v3 drift=9 ignored=text");
        assert!(!state.healthy);
        assert!(state.counters.is_empty());
        assert_eq!(state.sections["work.v1"].len(), 2);
        assert_eq!(state.sections.len(), 1);
    }
}
