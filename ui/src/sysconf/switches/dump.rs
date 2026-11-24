// SPDX-License-Identifier: GPL-2.0-or-later

use super::{conf_value, text, Env};
use std::{ffi::OsStr, sync::Arc};

/// Startup configuration for the bounded acquisition fixture dump.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BatchDump {
    on: bool,
    count: usize,
    dir: Option<Arc<OsStr>>,
}

impl BatchDump {
    pub(crate) fn parse(env: Env, conf: &str) -> Self {
        let override_on = env("OOS_BATCH_DUMP").map(|value| if value == "1" || value == "on" { "on" } else { "off" });
        Self {
            on: conf_value(override_on, conf, "acq.dump").as_deref() == Some("on"),
            count: conf_value(text(env, "OOS_BATCH_DUMP_COUNT").as_deref(), conf, "acq.dump.count")
                .and_then(|value| value.parse().ok())
                .unwrap_or(4),
            dir: env("OOS_BATCH_DUMP_DIR").map(Arc::from),
        }
    }

    pub(crate) fn on(&self) -> bool {
        self.on
    }

    #[cfg(any(target_os = "linux", test))]
    pub(crate) fn destination(&self) -> Option<(&std::path::Path, usize)> {
        self.on.then(|| {
            let dir = self.dir.as_deref().unwrap_or_else(|| OsStr::new("/tmp/oos-batches"));
            (std::path::Path::new(dir), self.count)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn batch_dump_is_opt_in_and_uses_startup_overrides() {
        let none = |_: &str| None;
        let dest = |count| Some((Path::new("/tmp/oos-batches"), count));
        assert_eq!(BatchDump::parse(&none, "").destination(), None);
        assert_eq!(BatchDump::parse(&none, "acq.dump = on").destination(), dest(4));
        let conf = "acq.dump = on\nacq.dump.count = 7 # per change";
        assert_eq!(BatchDump::parse(&none, conf).destination(), dest(7));
        let off = |key: &str| (key == "OOS_BATCH_DUMP").then(|| "0".into());
        assert_eq!(BatchDump::parse(&off, conf).destination(), None);
        let env = |key: &str| match key {
            "OOS_BATCH_DUMP" => Some("1".into()),
            "OOS_BATCH_DUMP_COUNT" => Some("2".into()),
            "OOS_BATCH_DUMP_DIR" => Some("/tmp/fixture-output".into()),
            _ => None,
        };
        let conf = "acq.dump = off\nacq.dump.count = 7";
        assert_eq!(BatchDump::parse(&env, conf).destination(), Some((Path::new("/tmp/fixture-output"), 2)));
    }
}
