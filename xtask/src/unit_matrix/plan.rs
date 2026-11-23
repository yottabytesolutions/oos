// SPDX-License-Identifier: GPL-2.0-or-later

mod bench;

use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

pub(super) type Setup = BTreeMap<String, String>;

#[derive(Debug, Deserialize)]
pub(super) struct Plan {
    pub(super) base_setup: Setup,
    bench_setup: Setup,
    snapshot_only: Vec<String>,
    groups: Vec<Group>,
}

#[derive(Debug, Deserialize)]
pub(super) struct Group {
    pub(super) id: String,
    pub(super) windows: usize,
    pub(super) overrides: Setup,
}

impl Plan {
    pub(super) fn load() -> Result<Self, String> {
        let plan: Self = serde_json::from_str(include_str!("../../data/unit-matrix.json"))
            .map_err(|e| format!("invalid unit matrix data: {e}"))?;
        let keys: BTreeSet<_> = plan.keys().collect();
        let ids: BTreeSet<_> = plan.groups.iter().map(|group| group.id.as_str()).collect();
        if keys.len() != plan.base_setup.len() + plan.bench_setup.len() + plan.snapshot_only.len()
            || ids.len() != plan.groups.len()
        {
            return Err("duplicate setup key or group in unit matrix data".into());
        }
        if plan.base_setup.is_empty()
            || plan.bench_setup.is_empty()
            || keys.iter().any(|key| key.as_str() == "bode")
            || plan.groups.is_empty()
            || plan
                .groups
                .iter()
                .any(|group| group.windows == 0 || group.overrides.keys().any(|key| !plan.base_setup.contains_key(key)))
        {
            return Err("invalid group or setup in unit matrix data".into());
        }
        Ok(plan)
    }

    pub(super) fn select(&self, ids: &[&str]) -> Result<Vec<&Group>, String> {
        let groups: Vec<_> =
            self.groups.iter().filter(|group| ids.is_empty() || ids.contains(&group.id.as_str())).collect();
        if !ids.is_empty() && ids.iter().copied().collect::<BTreeSet<_>>().len() != groups.len() {
            return Err("unknown --group; see xtask/data/unit-matrix.json for group IDs".into());
        }
        Ok(groups)
    }

    pub(super) fn keys(&self) -> impl Iterator<Item = &String> {
        self.base_setup.keys().chain(self.bench_setup.keys()).chain(&self.snapshot_only)
    }

    pub(super) fn default_target(&self) -> Setup {
        let mut target = self.base_setup.clone();
        target.extend(self.bench_setup.clone());
        target
    }

    pub(super) fn target(&self, group: &Group) -> Setup {
        let mut target = self.default_target();
        target.extend(group.overrides.clone());
        target
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archived_groups_keep_order_and_use_channel_two_for_triggering() {
        let plan = Plan::load().unwrap();
        let groups = plan.select(&[]).unwrap();
        assert_eq!(groups.len(), 26);
        assert_eq!(groups[0].id, "drift-before");
        assert_eq!(groups[25].id, "drift-after");
        assert_eq!(groups[25].windows, 1);
        for group in groups {
            let target = plan.target(group);
            assert_eq!(target["trig_src"], "1");
            assert_eq!(target["ch2.on"], "true");
            if group.id.ends_with("-fastest") {
                assert_eq!(target["tdiv"], target["zoom_tdiv"]);
            }
        }
        assert!(plan.select(&["missing"]).is_err());
        assert!(plan.keys().any(|key| key == "zone2"));
        assert!(!plan.keys().any(|key| key == "draw_path"));
        assert!(!plan.keys().any(|key| key.starts_with("bode")));
    }
}
