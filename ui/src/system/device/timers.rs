// SPDX-License-Identifier: GPL-2.0-or-later
//! Device process wiring, extracted from main.
use crate::*;
use std::time::Instant;

pub(super) struct Timers {
    pub(super) last: Instant,
    fps_t: Instant,
    fps_n: u64,
    shot_n: u32,
    led_mask: u32,
    led_t: Instant,
    net_t: Option<Instant>,
    eth_t0: Instant,
    eth_t: Option<Instant>,
    start_up_t: Instant,
    start_up: Option<String>,
}
impl Timers {
    pub(super) fn new(start_up: Option<String>) -> Self {
        let now = Instant::now();
        Self {
            last: now,
            fps_t: now,
            fps_n: 0,
            shot_n: 0,
            led_mask: u32::MAX,
            led_t: now,
            net_t: None,
            eth_t0: now,
            eth_t: None,
            start_up,
            start_up_t: now.checked_sub(std::time::Duration::from_millis(300)).unwrap_or(now),
        }
    }
    pub(super) fn startup_status(&mut self, ui: &mut Ui) {
        // the start-up file, every 250 ms while the screen is up
        if let Some(p) =
            self.start_up.as_deref().filter(|_| ui.start_up_active() && self.start_up_t.elapsed().as_millis() >= 250)
        {
            self.start_up_t = Instant::now();
            let up = std::fs::read_to_string("/proc/uptime")
                .ok()
                .and_then(|s| s.split_whitespace().next().and_then(|t| t.parse().ok()))
                .unwrap_or(0.0);
            ui.start_up_update(&std::fs::read_to_string(p).unwrap_or_default(), up);
        }
    }
    pub(super) fn panel(&mut self, ui: &mut Ui, cv: &Canvas, inp: &mut input::Inputs) {
        use board::Board;
        // LEDs: on change, and refreshed every 2 s (the MCU may have been reset)
        let lm = ui.leds();
        if lm != self.led_mask || self.led_t.elapsed().as_secs() >= 2 {
            inp.set_leds(lm);
            self.led_mask = lm;
            self.led_t = Instant::now();
        }
        if let Some(p) = ui.shot_request.take() {
            save_shot(ui, cv, p, &mut self.shot_n);
        }
        for c in ui.panel_tx.drain(..) {
            inp.send(&board::board().panel_cmd(c));
        }
    }
    pub(super) fn network(&mut self, ui: &mut Ui) {
        if self.net_t.is_none_or(|t| t.elapsed().as_secs() >= 5) {
            let (first, all) = ipv4_addresses();
            (ui.net_if, ui.net) = first;
            ui.net_ifs = all;
            ui.net_fixed = std::path::Path::new("/tmp/oos-net.fallback").exists();
            ui.eth_link = std::fs::read_to_string("/sys/class/net/eth0/carrier").is_ok_and(|c| c.trim() == "1");
            self.net_t = Some(Instant::now());
        }
        if self.eth_t.is_none_or(|t| t.elapsed().as_secs() >= 1) {
            self.eth_t = Some(Instant::now());
            eth_watch(ui, self.eth_t0.elapsed().as_secs_f64());
        }
        self.fps_n += 1;
        if self.fps_t.elapsed().as_secs_f64() >= 1.0 {
            ui.fps = self.fps_n as f32 / self.fps_t.elapsed().as_secs_f32();
            self.fps_n = 0;
            self.fps_t = Instant::now();
        }
    }
}
