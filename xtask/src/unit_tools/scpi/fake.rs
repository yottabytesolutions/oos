// SPDX-License-Identifier: GPL-2.0-or-later

use super::{Scpi, TimedReply, STATE};
use std::{cell::RefCell, collections::VecDeque};

pub(crate) type Events = RefCell<Vec<String>>;

pub(crate) struct FakeScpi<'a> {
    pub(crate) events: &'a Events,
    pub(crate) running: bool,
    pub(crate) fail_stop: bool,
    pub(crate) ignore_stop: bool,
    pub(crate) fail_run: usize,
    pub(crate) replies: VecDeque<(String, Result<String, String>)>,
}

impl Scpi for FakeScpi<'_> {
    fn send(&mut self, command: &str) -> Result<(), String> {
        self.events.borrow_mut().push(command.into());
        match command {
            ":STOP" if self.fail_stop => return Err("stop write failed".into()),
            ":STOP" if self.ignore_stop => return Ok(()),
            ":RUN" if self.fail_run > 0 => {
                self.fail_run -= 1;
                return Err("run write failed".into());
            }
            _ => {}
        }
        self.running = match command {
            ":STOP" => false,
            ":RUN" => true,
            _ => panic!("unexpected SCPI command"),
        };
        Ok(())
    }

    fn query_timed<T>(&mut self, command: &str, mut now: impl FnMut() -> T) -> Result<TimedReply<T>, String> {
        let before_write = now();
        let line = if let Some((expected, reply)) = self.replies.pop_front() {
            assert_eq!(command, expected);
            self.events.borrow_mut().push(command.into());
            reply?
        } else {
            assert_eq!(command, STATE);
            self.events.borrow_mut().push("query state".into());
            let mode = if self.running { "Run" } else { "Stop" };
            format!("fps 0 frames 0 records 0 run {mode} watchdog ok")
        };
        Ok(TimedReply { line, before_write, after_line: now() })
    }

    fn reconnect(&mut self) -> Result<(), String> {
        self.events.borrow_mut().push("reconnect".into());
        Ok(())
    }
}
