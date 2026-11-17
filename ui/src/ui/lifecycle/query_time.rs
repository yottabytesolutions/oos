// SPDX-License-Identifier: GPL-2.0-or-later
//! Construction-time clock and wait inputs for bounded measurement queries.
use super::Ui;
use std::{rc::Rc, time::Instant};

pub(in crate::ui) struct QueryTime {
    pub(in crate::ui) now: Rc<dyn Fn(&Ui) -> Instant>,
    pub(in crate::ui) wait: Rc<dyn Fn(&Ui)>,
}
impl Default for QueryTime {
    fn default() -> Self {
        Self {
            now: Rc::new(|_| Instant::now()),
            wait: Rc::new(|_| std::thread::sleep(std::time::Duration::from_millis(1))),
        }
    }
}
