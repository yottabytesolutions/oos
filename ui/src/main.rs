// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! oos-ui: the OOS oscilloscope screen.
//!
//!   oos-ui                         on the scope: DRM/KMS display, touch, front panel, control FIFO
//!   oos-ui --script FILE           anywhere: run a command script off-screen and write screenshots (tests)
//!   oos-ui --replay FRAME.bin --pre "<:WAV:PRE? string>"   show a captured raw frame (CH1) instead of the demo source
//!   --store-dir DIR                storage directory for setups, CSV and PNG (default /tmp/oos; scripts: the script's dir)
//!   --model NAME                   act as this model (DHO802 .. DHO924S), instead of /rigol/data/vendor.bin
//!   --limits FILE                  experimental limits (default /oos/limits.conf, docs/models.md)

// SCPI, web remote, display and the live source only run on the scope (Linux); the host build is the test runner.
#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

mod board;
mod clock;
mod cmd;
mod compress;
mod decode;
mod deepmeas;
mod drawpipe;
#[cfg(target_os = "linux")]
mod drm;
mod ethlink;
mod fastdraw;
mod font;
mod framecost;
mod framestats;
mod gfx;
mod gpu;
mod hostname;
mod http;
#[cfg(target_os = "linux")]
mod input;
mod json;
mod layout;
mod math;
mod mdns;
mod model;
mod net;
mod net6;
mod netacl;
mod pacing;
mod perfguard;
mod placement;
mod protocols;
mod scpi;
mod sntp;
mod sparse;
mod ssh;
mod store;
mod sysconf;
mod system;
mod trig;
mod trignear;
mod ui;
mod usbtmc;
mod vpu;
mod vxi11;
mod wave;
mod zone;

use gfx::Canvas;
pub use ui::Ui;
use wave::Source;

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1).cloned())
}

fn source(args: &[String], switches: &sysconf::Switches) -> Source {
    #[cfg(target_os = "linux")]
    if args.iter().any(|a| a == "--fpga-live") {
        use board::Board;
        let pre = arg(args, "--pre").unwrap_or_else(|| "1,2,4000,1,8e-10,0,0,1.3333e-4,-11250,32768".into());
        let p: Vec<f64> = pre.trim().split(',').map(|s| s.trim().parse().unwrap_or(0.0)).collect();
        if let Some(shared) = board::board().live(args) {
            return Source::Live { shared, seen: 0, words: Vec::new(), xinc: p[4], yinc: p[7], yorg: p[8], yref: p[9] };
        }
    }
    #[cfg(target_os = "linux")]
    if args.iter().any(|a| a == "--hw") {
        use board::Board;
        let mask =
            arg(args, "--app-mask").and_then(|v| u32::from_str_radix(v.trim_start_matches("0x"), 16).ok()).unwrap_or(1);
        let cal_dir = arg(args, "--cal").unwrap_or_else(|| "/rigol/data".into());
        eprintln!("oos-ui: board {}", board::board().name());
        // on the unit the frame loop and the acquisition thread keep the two A72 for themselves
        http::pin_to_big_cores(switches.cores);
        placement::place(placement::Role::Ui, switches);
        let program_afe = args.iter().any(|a| a == "--afe");
        let o = board::Spawn { mask, program_afe, cal_dir, start_up: arg(args, "--start-up") };
        let shared = board::board().spawn(o, *switches);
        return Source::Hw { shared, seen: 0 };
    }
    if let Some(path) = arg(args, "--replay") {
        let raw = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {}", path, e));
        let words: Vec<u16> = raw.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        // :WAV:PRE? = format,type,points,count,xinc,xorigin,xref,yinc,yorigin,yref
        let pre = arg(args, "--pre").unwrap_or_else(|| "1,2,4000,1,8e-10,0,0,1.3333e-4,-11250,32768".into());
        let p: Vec<f64> = pre.trim().split(',').map(|s| s.trim().parse().unwrap_or(0.0)).collect();
        return Source::Replay { words, xinc: p[4], yinc: p[7], yorg: p[8], yref: p[9] };
    }
    #[cfg(not(target_os = "linux"))]
    let _ = switches;
    Source::Demo { t: 0.0 }
}

pub fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--headless-script") {
        return Ui::start_headless_script(&args);
    }
    if args.iter().any(|a| a == "--scripted-batch") {
        eprintln!("oos-ui: --scripted-batch requires --headless-script");
        std::process::exit(2);
    }
    if args.iter().any(|a| a == "--fpga-dry") {
        // print the register writes of the live source (no hardware access)
        use board::Board;
        board::board().dry();
        return;
    }
    // the unit: model from /rigol/data/vendor.bin, standard limits plus /oos/limits.conf (docs/models.md);
    let limits = arg(&args, "--limits").unwrap_or_else(|| oos_hal::model::LIMITS_CONF.into());
    let p = oos_hal::model::Platform::detect(
        &arg(&args, "--cal").unwrap_or_else(|| "/rigol/data".into()),
        &limits,
        arg(&args, "--model").as_deref(),
    );
    eprintln!("oos-ui: {} ({})", p.ident(), p.caps_line());
    for u in &p.unit {
        eprintln!("oos-ui: unit default {} ({})", u, oos_hal::model::UNIT_CONF);
    }
    for o in &p.overrides {
        eprintln!("oos-ui: EXPERIMENTAL limit {}", o);
    }
    for w in &p.warnings {
        eprintln!("oos-ui: {}", w);
    }
    let experimental = p.experimental();
    let foreign = p.foreign.clone();
    oos_hal::model::init(p);
    // another board (docs/porting.md): no hardware source, the demo signal instead
    let args: Vec<String> = match &foreign {
        Some(_) => args.iter().filter(|a| !matches!(a.as_str(), "--hw" | "--fpga-live" | "--afe")).cloned().collect(),
        None => args,
    };
    let startup = store::startup::Startup::device(&args);
    let switches = sysconf::Switches::read();
    let mut ui = Ui::new(source(&args, &switches), ui::host::Host::system(switches));
    if let Some(m) = &foreign {
        ui.toast(&format!(
            "This is a {}: OOS drives DHO800/DHO900 hardware only. Demo signal shown (docs/porting.md)",
            m
        ));
    }
    ui.limits_path = limits;
    if experimental {
        ui.toast("Experimental limits active (limits.conf), see System > System info");
    }
    if let Some(script) = arg(&args, "--script") {
        ui.start_script(&script, &args);
        return;
    }
    if let Some(d) = arg(&args, "--store-dir") {
        ui.store_dir = d.into();
    }
    if args.iter().any(|a| a == "--serve") {
        return serve(ui, &switches);
    }
    #[cfg(target_os = "linux")]
    {
        system::device::run(ui, &args, switches, startup);
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (ui, startup);
        eprintln!("oos-ui: no display backend on this OS; use --script FILE");
    }
}

/// No display and no hardware: the demo source behind SCPI, the web server and VXI-11, for tools/device-test.py on a
/// host. `OOS_DEMO_LOOP=<mask>` puts the generator on those channels, as a cable from the output would.
fn serve(mut ui: Ui, sw: &sysconf::Switches) {
    let mut web = http::Http::open(net6::Port::Host, sw);
    let mut scpi = scpi::Scpi::open(net6::Port::Host, false);
    let mut vxi = net6::host_or_exit(vxi11::Server::host());
    net6::host_or_exit(net6::host_ready(web.port(), scpi.port(), vxi.ports()));
    let mut cv = Canvas::new(ui.scr.w, ui.scr.h);
    while !ui.quit {
        ui.step(0.04);
        ui.render(&mut cv);
        scpi.poll(&mut ui, &cv);
        vxi.poll(&mut |c| scpi.exec_line(&mut ui, &cv, c));
        for c in web.poll(&mut ui, &cv, &mut scpi) {
            cmd::run(&mut ui, &c);
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    system::stop_server(&mut ui, &mut web, &mut scpi, &mut vxi);
}

/// The layers of a GPU picture (System > Graphics = GPU): under and over the traces, and the traces as jobs. The
/// picture itself is in the display's buffer; it comes back into the caller's canvas only when asked.
struct GpuLayers {
    under: Canvas,
    over: Canvas,
    jobs: Vec<ui::TraceJob>,
}

impl GpuLayers {
    /// The layers record commands that the GPU draws (`gfx::Canvas::record`); `OOS_GPU_PIXELS=1`: they are drawn
    /// on the CPU and uploaded as pixels, as before (for comparison).
    fn new(w: i32, h: i32) -> GpuLayers {
        let (mut under, mut over) = (Canvas::new(w, h), Canvas::new_layer(w, h));
        let rec = std::env::var_os("OOS_GPU_PIXELS").is_none();
        under.record(rec);
        over.record(rec);
        GpuLayers { under, over, jobs: Vec::new() }
    }

    /// The screen in layers at `w` x `h`: the panel's, or (`view`) the monitor's own layout.
    fn draw(&mut self, ui: &mut Ui, w: i32, h: i32, view: bool) {
        if (self.under.w, self.under.h) != (w, h) {
            *self = GpuLayers::new(w, h);
        }
        let mut l = ui::Layers { under: &mut self.under, over: Some(&mut self.over), jobs: Vec::new() };
        if view {
            ui.render_view_layers(&mut l);
        } else {
            ui.render_layers(&mut l);
        }
        self.jobs = std::mem::take(&mut l.jobs);
    }

    fn quad<'a>(&'a self, ui: &'a Ui, src: gfx::Rect, dst: gfx::Rect, filter: bool, reuse: bool) -> gpu::Quad<'a> {
        gpu::Quad {
            under: Some(&self.under),
            over: Some(&self.over),
            size: (self.under.w, self.under.h),
            src,
            dst,
            filter,
            reuse,
            jobs: &self.jobs,
            ph: &ui.ph,
            phz: &ui.phz,
        }
    }

    /// The panel's picture into `target` (the size of `cv`); with `read` it also comes back into `cv` (the web page,
    /// screenshots, SCPI).
    fn compose(
        &mut self,
        ui: &mut Ui,
        g: &mut gpu::Gpu,
        cv: &mut Canvas,
        target: usize,
        read: bool,
    ) -> Result<(), String> {
        self.draw(ui, cv.w, cv.h, false);
        let full = gfx::Rect::new(0, 0, cv.w, cv.h);
        g.compose(target, None, &[self.quad(ui, full, full, false, false)], read.then_some(cv))
    }
}

/// The monitor's picture on the GPU (Display > HDMI with Graphics = GPU), right after the panel's: Mirror and
/// Waveform take the panel's layers again without a new upload (quad 0); Layout draws the monitor's own layers.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
#[allow(clippy::too_many_arguments)]
fn hdmi_gpu_frame(
    ui: &mut Ui,
    g: &mut gpu::Gpu,
    panel: &GpuLayers,
    view: &mut GpuLayers,
    head: &mut Option<Canvas>,
    mw: i32,
    mh: i32,
    target: usize,
    read: bool,
) -> Result<gpu::Ticket, String> {
    let whole = gfx::Rect::new(0, 0, ui.scr.w, ui.scr.h);
    let fit_of = |sw: i32, sh: i32, dw: i32, dh: i32| {
        let f = gfx::Fit::new(sw, sh, dw, dh);
        let (w, h) = f.size();
        gfx::Rect::new(f.x0 as i32, f.y0 as i32, w as i32, h as i32)
    };
    let mirror = |g: &mut gpu::Gpu| {
        g.submit(target, Some(0), &[panel.quad(ui, whole, fit_of(whole.w, whole.h, mw, mh), true, true)], read)
    };
    match ui.m.hdmi {
        model::HdmiMode::Layout => match ui::Screen::for_monitor(mw, mh) {
            Some(s) => {
                view.draw(ui, s.w, s.h, true);
                // quad 0 stays the panel's: an empty one, the monitor's layers go to quad 1
                let none = gpu::Quad {
                    under: None,
                    over: None,
                    size: (1, 1),
                    src: gfx::Rect::new(0, 0, 1, 1),
                    dst: gfx::Rect::new(0, 0, 0, 0),
                    filter: false,
                    reuse: false,
                    jobs: &[],
                    ph: &ui.ph,
                    phz: &ui.phz,
                };
                let q = view.quad(ui, gfx::Rect::new(0, 0, s.w, s.h), gfx::Rect::new(0, 0, mw, mh), true, false);
                g.submit(target, None, &[none, q], read)
            }
            None => mirror(g),
        },
        model::HdmiMode::Waveform => {
            // as `hdmi_workspace`: the settings line in a band on top, the waveform area as large as it fits under it
            let body = ui.frame().body;
            let (sw, sh) = (ui.scr.w, body.h);
            let band = 48.max((mh - mw * sh / sw) / 2);
            let r = fit_of(sw, sh, mw, mh - band);
            let hc = head.get_or_insert_with(|| Canvas::new(mw, band));
            if (hc.w, hc.h) != (mw, band) {
                *hc = Canvas::new(mw, band);
            }
            hc.clear(HDMI_BG);
            let f = font::Fonts::get(0);
            hc.text_in(&f.xl, gfx::Rect::new(0, 0, mw, band), 1, &ui.hdmi_header(), gfx::rgb(0xe6, 0xed, 0xf3));
            // the waveform area without the bars and menus: the layer over the traces as it was before them
            let over =
                ui.hdmi_over.as_ref().filter(|o| (o.w, o.h) == (panel.under.w, panel.under.h)).unwrap_or(&panel.over);
            let plot = gpu::Quad {
                over: Some(over),
                reuse: false,
                ..panel.quad(ui, body, gfx::Rect::new(r.x, band, r.w, r.h), true, false)
            };
            let line = gpu::Quad {
                under: Some(hc),
                over: None,
                size: (mw, band),
                src: gfx::Rect::new(0, 0, mw, band),
                dst: gfx::Rect::new(0, 0, mw, band),
                filter: false,
                reuse: false,
                jobs: &[],
                ph: &ui.ph,
                phz: &ui.phz,
            };
            g.submit(target, Some(HDMI_BG), &[plot, line], read)
        }
        model::HdmiMode::Mirror => mirror(g),
    }
}

/// Background of the HDMI workspace.
const HDMI_BG: u32 = 0x0d1117;

/// The GPU helper with the panel's scan-out buffers as targets 0 to 2 (the monitor's follow them, up to the helper's 8).
#[cfg(target_os = "linux")]
fn start_gpu(disp: &drm::Display, sw: &sysconf::Switches) -> Result<gpu::Gpu, String> {
    http::pin_to_big_cores(sw.cores);
    placement::place(placement::Role::Ui, sw);
    let mut g = gpu::Gpu::start(gpu::HELPER)?;
    for i in 0..disp.buffers() {
        let (fd, pitch) = disp.export(i).map_err(|e| format!("export: {}", e))?;
        g.target_dmabuf(i, disp.width, disp.height, pitch, fd)?;
    }
    g.monitor_targets_from(disp.buffers());
    Ok(g)
}

/// Writes a requested screenshot of the frame just rendered: "" = /tmp/oos-shot-N.ppm, *.png = PNG (storage, the
/// toast shows the file name), else PPM.
fn save_shot(ui: &mut Ui, cv: &Canvas, p: String, n: &mut u32) {
    let path = if p.is_empty() {
        *n += 1;
        format!("/tmp/oos-shot-{}.ppm", n)
    } else {
        p
    };
    let png = path.ends_with(".png");
    let r = if png { store::write(std::path::Path::new(&path), &store::png_of(cv)) } else { cmd::write_ppm(cv, &path) };
    match r {
        Ok(()) if png => {
            ui.flush_stick();
            ui.toast(&format!("Saved {}{}", store::file_name(std::path::Path::new(&path)), ui.saved_where()));
        }
        Ok(()) => ui.toast(&format!("Saved {}", path)),
        Err(e) => ui.toast(&format!("Screenshot failed: {}", e)),
    }
}

/// Power-on pictures, first found wins: a picture copied by rcS from removable media, then the image's own.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
const SPLASH: [&str; 2] = ["/tmp/oos-splash.ppm", "/etc/oos/splash.ppm"];

/// Power off / restart through the front-panel MCU, as the stock system does (board.md 4/8.3): file systems synced
/// first, LEDs off, then `FA 05 01` twice (power off) or `FA 05 03` (restart). Whether the MCU waits for Linux to halt
/// is not known; OOS keeps nothing on the card open for writing except during an update job (`Ui::update_busy`, which
/// refuses the power-off), so a hard cut is harmless.
#[cfg(target_os = "linux")]
fn power(inp: &mut input::Inputs, a: ui::PowerAction) {
    use board::Board;
    eprintln!("oos-ui: power {:?}", a);
    inp.set_leds(0);
    unsafe { libc::sync() };
    match a {
        ui::PowerAction::Off => {
            inp.send(&board::board().panel_cmd(board::PanelCmd::PowerOff));
            inp.send(&board::board().panel_cmd(board::PanelCmd::PowerOff));
        }
        ui::PowerAction::Restart => inp.send(&board::board().panel_cmd(board::PanelCmd::Restart)),
    }
}

/// Once a second: a link at 10 Mbit/s gets one more autonegotiation (System > Network > 10 Mbit retry, ethlink.rs).
#[cfg(target_os = "linux")]
fn eth_watch(ui: &mut Ui, now: f64) {
    let speed = ethlink::speed();
    match ui.eth.step(now, ethlink::carrier(), speed, ui.sys.eth_retry_10m) {
        Some(ethlink::Act::Restart) => {
            eprintln!(
                "oos-ui: {} up at 10 Mbit/s {}, restarting the autonegotiation ({} of {}; {})",
                ethlink::IFACE,
                ethlink::duplex(),
                ui.eth.tries,
                ethlink::MAX_TRIES,
                ethlink::mii_regs()
            );
            if let Err(e) = ethlink::restart() {
                eprintln!("oos-ui: {}: autonegotiation restart failed: {}", ethlink::IFACE, e);
            }
        }
        Some(ethlink::Act::Result(s)) => {
            let s = s.map_or("no speed".to_string(), |s| format!("{} Mbit/s", s));
            eprintln!(
                "oos-ui: {} after the restart: {} {} ({})",
                ethlink::IFACE,
                s,
                ethlink::duplex(),
                ethlink::mii_regs()
            );
        }
        None => {}
    }
}

/// The HDMI workspace: a settings line on top, below it the waveform area of the panel (plot and measurement rows,
/// without the bars and menus) scaled to the monitor with its aspect ratio.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn hdmi_workspace(ui: &Ui, hc: &mut Canvas) {
    hc.clear(HDMI_BG);
    let body = ui.frame().body;
    let (sw, sh) = (ui.scr.w, body.h);
    if ui.hdmi_snap.len() < (sw * sh) as usize {
        return;
    }
    // the waveform area as large as it fits; the settings line in the band above it (at least 48 px)
    let head = 48.max((hc.h - hc.w * sh / sw) / 2);
    let fit = gfx::Fit::new(sw, sh, hc.w, hc.h - head);
    let stride = hc.w as usize;
    // the monitor is large: its header keeps the normal text size
    let f = font::Fonts::get(0);
    hc.text_in(&f.xl, gfx::Rect::new(0, 0, hc.w, head), 1, &ui.hdmi_header(), gfx::rgb(0xe6, 0xed, 0xf3));
    // `fit` centres in the rest: start at the band so the plot sits right under the settings line
    let top = (head as usize).saturating_sub(fit.y0) * stride;
    fit.scale(&ui.hdmi_snap, &mut hc.px[top..], stride);
}

/// First IPv4 address of a non-loopback interface that is up ("" when none).
#[cfg(target_os = "linux")]
/// The address to show in the top bar, and all of them, as (interface, address).
fn ipv4_addresses() -> ((String, String), Vec<(String, String)>) {
    // an interface with a link first: eth0 keeps its fallback address without a cable
    let mut out = (String::new(), String::new());
    let mut all = Vec::new();
    let mut linked = false;
    unsafe {
        let mut ifa: *mut libc::ifaddrs = std::ptr::null_mut();
        if libc::getifaddrs(&mut ifa) != 0 {
            return (out, all);
        }
        let mut p = ifa;
        while !p.is_null() {
            let a = &*p;
            let up = a.ifa_flags & (libc::IFF_UP as u32) != 0 && a.ifa_flags & (libc::IFF_LOOPBACK as u32) == 0;
            if up && !a.ifa_addr.is_null() && (*a.ifa_addr).sa_family as i32 == libc::AF_INET {
                let link = a.ifa_flags & (libc::IFF_RUNNING as u32) != 0;
                let sin = &*(a.ifa_addr as *const libc::sockaddr_in);
                let name = std::ffi::CStr::from_ptr(a.ifa_name).to_string_lossy().into_owned();
                let addr = std::net::Ipv4Addr::from(u32::from_be(sin.sin_addr.s_addr)).to_string();
                if out.1.is_empty() || (link && !linked) {
                    out = (name.clone(), addr.clone());
                    linked = link;
                }
                all.push((name, addr));
            }
            p = a.ifa_next;
        }
        libc::freeifaddrs(ifa);
    }
    (out, all)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The HDMI workspace on a 1920 x 1080 monitor: settings line on top, the waveform area below without menus.
    #[test]
    fn hdmi_workspace_picture() {
        hdmi_workspace_picture_with_shots(None);
    }

    #[test]
    #[ignore = "manual screenshots: set OOS_SHOT_DIR"]
    fn hdmi_workspace_picture_manual_screenshots() {
        let dir = crate::store::tests::test_output_dir("OOS_SHOT_DIR");
        hdmi_workspace_picture_with_shots(Some(std::path::Path::new(&dir)));
    }

    fn hdmi_workspace_picture_with_shots(dir: Option<&std::path::Path>) {
        let mut ui = Ui::for_tests(Source::Demo { t: 0.0 });
        let mut cv = Canvas::new(ui::PANEL_W, ui::PANEL_H);
        ui.hdmi_snap_want = true;
        for _ in 0..10 {
            ui.step(1.0 / 30.0);
            ui.render(&mut cv);
        }
        let mut hc = Canvas::new(1920, 1080);
        hdmi_workspace(&ui, &mut hc);
        // the plot fills the width under the settings band (1024 x 486 -> 1920 x 911 under an 84-px band)
        assert_eq!(ui.hdmi_snap.len(), (ui.scr.w * ui.frame().body.h) as usize);
        assert!(ui.hdmi_header().contains("CH1 1 V/div"));
        if let Some(dir) = dir {
            std::fs::write(dir.join("hdmi-workspace.png"), crate::store::png_of(&hc)).unwrap();
        }
    }
}
