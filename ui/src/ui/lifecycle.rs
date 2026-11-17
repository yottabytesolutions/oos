// SPDX-License-Identifier: GPL-2.0-or-later
//! Process status and releasing the current buffer holders.

mod query_time;
#[cfg(test)]
pub(crate) mod test_wait;
pub(crate) mod worker_result;

use super::*;
use std::time::Instant;

pub(super) struct Runtime {
    pub(super) change: super::change::Observer,
    pub(super) memory: crate::system::memory::Watchdog,
    pub(super) crash: Option<crate::store::startup::crash::Crash>,
    pub(super) clocks: Option<crate::system::clocks::Handle>,
    started: Instant,
    pub(in crate::ui) query_time: query_time::QueryTime,
    pub(super) gpu: String,
    pub(super) persistence: bool,
    retired: Vec<std::thread::JoinHandle<()>>,
}

impl Runtime {
    pub(super) fn new(limits: crate::system::memory::Limits) -> Self {
        Self {
            change: Default::default(),
            memory: crate::system::memory::Watchdog::new(limits),
            crash: None,
            clocks: None,
            gpu: String::new(),
            persistence: true,
            retired: Vec::new(),
            started: Instant::now(),
            query_time: Default::default(),
        }
    }

    pub(super) fn text(&self) -> String {
        format!(
            "{} {} persistence {} last_crash {}",
            self.memory.status(),
            self.gpu,
            if self.persistence { "on" } else { "off" },
            self.crash.as_ref().map_or("none".into(), |c| c.text())
        )
    }
}

pub(super) fn stop_workers(ui: &mut Ui, deadline: Instant) {
    if let Some(clocks) = &ui.host.runtime().clocks {
        clocks.restore();
    }
    let mut joins = std::mem::take(&mut ui.host.runtime_mut().retired);
    if let Some(worker) = ui.meas_bg.take() {
        drop(worker.tx);
        joins.push(worker.join);
    }
    if let Some(pipe) = crate::drawpipe::pipe(crate::drawpipe::Access::Existing) {
        crate::drawpipe::shutdown::close(pipe, deadline);
    }
    crate::system::shutdown::join(joins, deadline);
}

pub(super) fn memory_step(ui: &mut Ui) -> bool {
    use crate::system::memory;
    // Thermal protection continues even when a prepared settings change is rejected.
    if let Some(clocks) = &ui.host.runtime().clocks {
        clocks.poll();
    }
    let now = ui.host.runtime().started.elapsed();
    let rss = if ui.host.runtime().memory.due(now) {
        ui.host.text("/proc/self/status").and_then(|s| memory::rss(&s))
    } else {
        None
    };
    apply_sample(ui, now, rss)
}

fn apply_sample(ui: &mut Ui, now: std::time::Duration, rss: Option<u64>) -> bool {
    use crate::system::memory::Action;
    let action = ui.host.runtime_mut().memory.sample(now, rss);
    match action {
        Action::Warn => {
            eprintln!("oos-ui: RSS soft limit: {}", ui.lifecycle_text());
            ui.toast("Memory use is high");
        }
        Action::Stop => {
            eprintln!("oos-ui: RSS hard limit: {}", ui.lifecycle_text());
            reset(ui);
            ui.toast("Memory limit: acquisition stopped");
        }
        Action::Exit75 => {
            eprintln!("oos-ui: RSS recovery deadline: exiting 75");
            ui.quit = true;
        }
        Action::None => {}
    }
    if ui.host.runtime().memory.stopped() {
        if ui.m.run == RunState::Run && action != Action::Stop {
            ui.host.runtime_mut().memory.resume();
        }
        if ui.host.runtime().memory.stopped() {
            ui.m.run = RunState::Stop;
            release_board(ui);
            return true;
        }
    }
    false
}

fn release_board(ui: &mut Ui) {
    if let Some(clocks) = &ui.host.runtime().clocks {
        clocks.update(false);
    }
    ui.src.hw_hold(true, ui.final_req);
    if let Source::Hw { shared, .. } = &ui.src {
        let mut shared = crate::perfguard::deep::ui_lock(shared);
        shared.queue.clear();
        shared.frame = Default::default();
        shared.deep_raw = None;
        shared.seg.shown = None;
    }
}

pub(super) fn reset(ui: &mut Ui) {
    ui.m.run = RunState::Stop;
    ui.was_running = false;
    ui.stop_final = None;
    if let Some(pipe) = crate::drawpipe::pipe(crate::drawpipe::Access::Existing) {
        crate::drawpipe::shutdown::reset(pipe);
    }
    release_board(ui);
    ui.last = [None];
    ui.history = Default::default();
    ui.hist_pos = None;
    ui.meas_pending = None;
    ui.meas_want = false;
    super::change::acquisition::cancel_measurement(ui);
    ui.ana = Default::default();
    let mut work = std::mem::take(&mut ui.azr.work);
    work.reset();
    ui.azr = Default::default();
    ui.azr.work = work;
    ui.seg_view = None;
    ui.refs = Default::default();
    ui.trend = Default::default();
    ui.pn = Default::default();
    ui.bus_views_clear();
    *ui.la_cols.get_mut() = Default::default();
    *ui.dec_cache.get_mut() = Vec::new();
    ui.mask_ref = None;
    ui.mask = None;
    ui.mask_cols = None;
    ui.hdmi_snap = Vec::new();
    ui.hdmi_over = None;
    ui.auto = None;
    // Drop channels/results, but let an in-flight measurement keep its owned record until completion.
    if let Some(worker) = ui.meas_bg.take() {
        drop(worker.tx);
        ui.host.runtime_mut().retired.push(worker.join);
    }
    ui.src.hw_needs_raw(false);
}

/// The measurements in the background, on the unit (`Ui::measure_in_background`): a thread on the slower cores
/// measures the newest record while the frame loop goes on, and its results are taken a frame or two later. A record
/// that comes while one is measured waits; a newer one replaces it. Without it (tests, scripts, the host) the
/// measurements are made in the frame they belong to.
pub(super) enum WorkerJob {
    Measure(MeasJob),
    Analyze(Box<analysis::window::Job>),
}

pub(super) struct MeasWorker {
    pub(super) tx: std::sync::mpsc::Sender<WorkerJob>,
    pub(super) analysis_rx: crate::ui::lifecycle::worker_result::Receiver<analysis::window::Done>,
    pub(super) analysis_busy: bool,
    pub(super) rx: crate::ui::lifecycle::worker_result::Receiver<MeasDone>,
    /// a job is out
    pub(super) busy: bool,
    pub(super) accept: bool,
    pub(super) join: std::thread::JoinHandle<()>,
}

impl MeasWorker {
    pub(super) fn start(little: u64) -> Option<MeasWorker> {
        let (tx, jobs) = std::sync::mpsc::channel::<WorkerJob>();
        let (analysis_done, analysis_rx) = crate::ui::lifecycle::worker_result::channel();
        let (done, rx) = crate::ui::lifecycle::worker_result::channel::<MeasDone>();
        let join = std::thread::Builder::new()
            .name("measure".into())
            .spawn(move || {
                crate::http::to_little_cores(little);
                let mut previous = None;
                while let Ok(job) = jobs.recv() {
                    let (n, cheap, work) = match job {
                        WorkerJob::Measure(work) => work,
                        WorkerJob::Analyze(job) => {
                            if analysis_done.send(analysis::window::run(*job, &mut previous)).is_err() {
                                break;
                            }
                            continue;
                        }
                    };
                    let w: Vec<(usize, &[f32], f64, f64)> =
                        work.iter().map(|(c, v, dt, min)| (*c, v.samples(), *dt, *min)).collect();
                    let t0 = std::time::Instant::now();
                    let r = measure_channels(&w, cheap);
                    // deep raw records (millions of samples): their cost on the state line (deepmeas::cost)
                    let pts: usize = w.iter().map(|x| x.1.len()).sum();
                    if pts > crate::compress::MAX_DEPTH as usize {
                        crate::deepmeas::cost::set(&crate::deepmeas::cost::MEASURE_US, t0.elapsed());
                        crate::deepmeas::cost::MEASURE_N.store(pts as u64, std::sync::atomic::Ordering::Relaxed);
                    }
                    if done.send((n, r)).is_err() {
                        break;
                    }
                }
            })
            .ok()?;
        Some(MeasWorker { tx, rx, analysis_rx, analysis_busy: false, busy: false, accept: false, join })
    }
}

pub(super) fn begin_step(ui: &mut Ui, dt_wall: f64) -> bool {
    ui.now += dt_wall;
    if ui.toast.as_ref().is_some_and(|(_, until)| ui.now > *until) {
        ui.toast = None;
    }
    change::is_prepared(ui) || polls(ui)
}

pub(in crate::ui) fn polls(ui: &mut Ui) -> bool {
    use crate::perfguard::deep::Poll;
    let stopped = change::poll(ui, Poll::Memory, memory_step);
    change::poll(ui, Poll::Relayout, Ui::relayout);
    change::poll(ui, Poll::MeasHold, Ui::meas_hold_tick);
    change::poll(ui, Poll::WorkspaceHold, Ui::workspace_hold_tick);
    change::poll(ui, Poll::Update, Ui::update_poll);
    change::poll(ui, Poll::Wlan, Ui::wlan_poll);
    change::poll(ui, Poll::Ntp, Ui::ntp_poll);
    change::poll(ui, Poll::Ssh, |u| {
        u.ssh_poll();
        u.allow_poll();
    });
    change::poll(ui, Poll::Usb, Ui::usb_poll);
    if stopped {
        return false;
    }
    change::poll(ui, Poll::ChHold, Ui::ch_hold_poll);
    change::poll(ui, Poll::Counter, Ui::counter_poll);
    change::poll(ui, Poll::Dvm, Ui::dvm_poll);
    change::poll(ui, Poll::Mask, Ui::mask_hw_poll);
    change::poll(ui, Poll::Zone, Ui::zone_hw_poll);
    change::poll(ui, Poll::LaProbe, Ui::la_probe_poll);
    change::poll(ui, Poll::Log, Ui::log_tick);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn memory_stop_keeps_clock_toasts_and_service_polls_running() {
        let mut ui = Ui::for_tests(Source::Demo { t: 0.0 });
        ui.host.runtime_mut().memory = crate::system::memory::Watchdog::new(crate::system::memory::Limits::parse(
            "mem.soft_mb = 1\nmem.hard_mb = 2\n",
        ));
        assert!(apply_sample(&mut ui, Duration::ZERO, Some(3 << 20)));
        ui.toast("Temporary notice");
        let before = ui.now;
        ui.step(3.0);
        assert!(ui.memory_stopped());
        assert_eq!(ui.now, before + 3.0);
        assert!(ui.toast.is_none(), "toasts expire while stopped");
        assert!(ui.last[0].is_none(), "no acquisition while memory is stopped");
    }

    #[test]
    fn persistence_off_is_visible_in_plot_system_page_and_state_query() {
        let mut ui = Ui::for_tests(Source::Demo { t: 0.0 });
        let mut before = Canvas::new(ui.scr.w, ui.scr.h);
        ui.draw_info(&mut before);
        ui.persistence_status(false);
        assert!(ui.lifecycle_text().contains("persistence off"));
        let rows = ui.sysinfo_rows();
        assert!(rows.iter().flatten().any(
            |r| matches!(r, super::super::sysinfo::Row::Note(t, _) if t == "Persistence off: storage unavailable")
        ));
        let mut after = Canvas::new(ui.scr.w, ui.scr.h);
        ui.draw_info(&mut after);
        assert_ne!(before.px, after.px, "the waveform status must change when storage is disabled");
    }

    #[test]
    fn hard_limit_releases_ui_records_without_requesting_a_final_read() {
        let mut ui = Ui::for_tests(Source::Demo { t: 0.0 });
        ui.step(0.04);
        assert!(ui.last[0].is_some());
        ui.hdmi_snap = vec![42; 1024];
        let final_req = ui.final_req;
        ui.host.runtime_mut().memory = crate::system::memory::Watchdog::new(crate::system::memory::Limits::parse(
            "mem.soft_mb = 1\nmem.hard_mb = 2\n",
        ));
        assert!(apply_sample(&mut ui, Duration::ZERO, Some(3 << 20)));
        assert_eq!(ui.m.run, RunState::Stop);
        assert_eq!(ui.final_req, final_req);
        assert_eq!(ui.hdmi_snap.capacity(), 0);
        assert!(ui.stop_final.is_none() && ui.last[0].is_none() && ui.history.is_empty());
        assert!(ui.meas_pending.is_none() && ui.refs.iter().all(Option::is_none));
        assert!(apply_sample(&mut ui, Duration::from_secs(5), Some(3 << 20)));
        assert!(ui.quit);
        assert_eq!(ui.exit_code(), 75);
    }

    #[test]
    fn hardware_stop_drops_published_frames_and_keeps_hold() {
        let shared: crate::board::HwShared = Default::default();
        let mut ui = Ui::for_tests(Source::Hw { shared: shared.clone(), seen: 0 });
        {
            let mut board = oos_hal::sync::lock(&shared);
            std::sync::Arc::make_mut(&mut board.frame).ch[0] = vec![42.0; 1024];
            board.queue.push_back(crate::board::Frame::default().into());
        }
        reset(&mut ui);
        let board = oos_hal::sync::lock(&shared);
        assert!(board.hold && board.queue.is_empty() && board.frame.ch[0].is_empty());
        assert_eq!(board.final_req, 0);
    }
}
