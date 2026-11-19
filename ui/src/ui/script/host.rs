// SPDX-License-Identifier: GPL-2.0-or-later

//! The off-screen script runner, shared by ordinary scripts and deterministic host captures.

pub mod hot_paths;
pub mod soak;

use super::batch::BatchScript;
use crate::board::scripted::BatchKind;
use crate::{cmd, gfx::Canvas, gpu, model, ui::Ui, GpuLayers};

pub fn start(ui: &mut Ui, path: &str, args: &[String]) {
    if let Err(e) = configure(ui, path, args).and_then(|gpu| run_script(ui, path, gpu, None, false)) {
        eprintln!("oos-ui: script: {}", e);
        std::process::exit(2);
    }
}

/// Enter before platform detection, source startup or reading host configuration.
pub fn headless(args: &[String]) {
    if let Err(e) = run_headless(args) {
        eprintln!("oos-ui: headless script: {}", e);
        std::process::exit(2);
    }
}

fn run_headless(args: &[String]) -> Result<(), String> {
    let conflicts = [
        "--hw",
        "--fpga-live",
        "--fpga-dry",
        "--afe",
        "--replay",
        "--pre",
        "--cal",
        "--limits",
        "--model",
        "--serve",
        "--gpu",
        "--size",
        "--start-up",
        "--app-mask",
    ];
    if let Some(a) = args.iter().find(|a| conflicts.contains(&a.as_str())) {
        return Err(format!("--headless-script conflicts with {}", a));
    }
    let path = crate::arg(args, "--script").filter(|p| !p.starts_with("--")).ok_or("--script FILE required")?;
    let kind = if args.iter().any(|a| a == "--scripted-batch") {
        Some(BatchKind::parse(&crate::arg(args, "--scripted-batch").ok_or("--scripted-batch KIND required")?)?)
    } else {
        None
    };
    oos_hal::model::init(oos_hal::model::Platform::build(None, None, Some("DHO924S")));
    let mut ui = Ui::new(crate::wave::Source::Demo { t: 0.0 }, crate::ui::host::Host::headless());
    configure(&mut ui, &path, args)?;
    run_script(&mut ui, &path, None, kind, true)
}

fn configure(ui: &mut Ui, path: &str, args: &[String]) -> Result<Option<gpu::Gpu>, String> {
    if let Some(d) = std::path::Path::new(path).parent().filter(|d| !d.as_os_str().is_empty()) {
        ui.store_dir = d.to_path_buf();
    }
    if let Some(d) = crate::arg(args, "--store-dir") {
        ui.store_dir = d.into();
    }
    if let Some(size) = crate::arg(args, "--size") {
        ui.scr = crate::ui::Screen::parse(&size).ok_or_else(|| script_size_error(&size))?;
    }
    crate::arg(args, "--gpu")
        .map(|helper| {
            let mut g = gpu::Gpu::start(&helper)?;
            g.target_memory(0, ui.scr.w, ui.scr.h)?;
            Ok(g)
        })
        .transpose()
}

fn script_size_error(size: &str) -> String {
    format!("--size {}: give WIDTHxHEIGHT, at least {}x{}", size, crate::ui::PANEL_W, crate::ui::PANEL_H)
}

pub struct HostScript {
    cv: Canvas,
    gpu: Option<gpu::Gpu>,
    panel: GpuLayers,
    monitor: GpuLayers,
    head: Option<Canvas>,
    hdmi: Option<Canvas>,
    shot_n: u32,
    batch: Option<BatchScript>,
    metadata: bool,
}

impl HostScript {
    fn new(ui: &mut Ui, mut gpu: Option<gpu::Gpu>, kind: Option<BatchKind>, metadata: bool) -> Result<Self, String> {
        let hdmi =
            std::env::var("OOS_GPU_HDMI").ok().and_then(|s| crate::ui::Screen::parse(&s)).filter(|_| gpu.is_some());
        if let Some(m) = gpu
            .as_ref()
            .and_then(|_| std::env::var("OOS_GPU_HDMI_MODE").ok())
            .and_then(|v| model::HDMI_MODES.iter().copied().find(|x| format!("{:?}", x) == v))
        {
            ui.m.hdmi = m;
        }
        if let (Some(g), Some(s)) = (gpu.as_mut(), hdmi) {
            g.target_memory(2, s.w, s.h)?;
        }
        ui.info = "host test run".into();
        Ok(Self {
            cv: Canvas::new(ui.scr.w, ui.scr.h),
            gpu,
            panel: GpuLayers::new(ui.scr.w, ui.scr.h),
            monitor: GpuLayers::new(1, 1),
            head: None,
            hdmi: hdmi.map(|s| Canvas::new(s.w, s.h)),
            shot_n: 0,
            batch: kind.map(|k| BatchScript::new(ui, k)),
            metadata,
        })
    }

    fn render_step(&mut self, ui: &mut Ui) -> Result<(), String> {
        if let Some(batch) = &mut self.batch {
            batch.feed_step(ui)?;
        }
        ui.step(1.0 / 30.0);
        if let Some(batch) = &mut self.batch {
            batch.finish_step(ui)?;
        }
        ui.hdmi_snap_want = self.hdmi.is_some();
        match self.gpu.as_mut() {
            Some(g) => {
                if let Err(e) = self.panel.compose(ui, g, &mut self.cv, 0, true) {
                    gpu_failure("frame", &e);
                }
            }
            None => ui.render(&mut self.cv),
        }
        if let Some(p) = ui.shot_request.take() {
            crate::save_shot(ui, &self.cv, p, &mut self.shot_n);
        }
        Ok(())
    }

    fn capture(&mut self, ui: &mut Ui, path: &str) -> Result<(), String> {
        self.render_step(ui)?;
        if path.ends_with(".png") {
            crate::store::write(std::path::Path::new(path), &crate::store::png_of(&self.cv))
        } else {
            cmd::write_ppm(&self.cv, path)
        }
        .map_err(|e| format!("write shot {}: {}", path, e))?;
        if self.metadata && path.ends_with(".png") {
            super::capture::metadata(ui, path)?;
        }
        println!("shot {}", path);
        self.capture_hdmi(ui, path)
    }

    fn capture_hdmi(&mut self, ui: &mut Ui, path: &str) -> Result<(), String> {
        let (Some(g), Some(hc)) = (self.gpu.as_mut(), self.hdmi.as_mut()) else { return Ok(()) };
        crate::hdmi_gpu_frame(ui, g, &self.panel, &mut self.monitor, &mut self.head, hc.w, hc.h, 2, true)
            .and_then(|ticket| g.collect(ticket, Some(hc)))
            .unwrap_or_else(|e| gpu_failure("HDMI", &e));
        let stem = path.strip_suffix(".ppm").or_else(|| path.strip_suffix(".png")).unwrap_or(path);
        cmd::write_ppm(hc, &format!("{}-hdmi.ppm", stem)).map_err(|e| e.to_string())?;
        // Retain the GPU parity runner's existing per-shot timing output.
        if let Some(f) = std::env::var_os("OOS_GPU_TIMES") {
            append_gpu_times(std::path::Path::new(&f), g.times.panel, g.times.monitor)?;
        }
        Ok(())
    }
}

fn append_gpu_times(path: &std::path::Path, panel: f32, monitor: f32) -> Result<(), String> {
    use std::io::Write;
    let line = format!("{:.3} {:.3}\n", panel, monitor);
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut f| f.write_all(line.as_bytes()))
        .map_err(|e| format!("write GPU times {}: {}", path.display(), e))
}

fn gpu_failure(area: &str, error: &str) -> ! {
    eprintln!("oos-ui: GPU {}: {}", area, error);
    std::process::exit(3);
}

pub fn run_script(
    ui: &mut Ui,
    path: &str,
    gpu: Option<gpu::Gpu>,
    kind: Option<BatchKind>,
    metadata: bool,
) -> Result<(), String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("read script {}: {}", path, e))?;
    let mut host = HostScript::new(ui, gpu, kind, metadata)?;
    // The mode comparison fixtures apply their settings before the first synthetic read.
    let (setup, script) = kind.and_then(|_| text.split_once("\n# first acquisition\n")).unwrap_or(("", &text));
    for line in setup.lines() {
        if !matches!(cmd::run(ui, line), cmd::Out::None) {
            return Err("scripted batch setup must precede frames and captures".into());
        }
    }
    host.render_step(ui)?;
    for line in script.lines() {
        match cmd::run(ui, line) {
            cmd::Out::Frames(n) => {
                for _ in 0..n {
                    host.render_step(ui)?;
                }
            }
            cmd::Out::Shot(p) => host.capture(ui, &p)?,
            cmd::Out::None => {}
        }
        let _ = super::super::lifecycle::storage::poll(ui, true);
        if ui.quit {
            break;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setup_save_completion_precedes_the_next_script_command() {
        let root = std::env::temp_dir().join(format!("oos-script-storage-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let script = root.join("save.txt");
        std::fs::write(&script, "menu Storage\ndo Save setup\n").unwrap();
        let mut ui = Ui::for_tests(crate::wave::Source::Demo { t: 0.0 });
        ui.store_dir = root.clone();
        ui.m.tscale = 12;
        run_script(&mut ui, script.to_str().unwrap(), None, None, false).unwrap();
        assert!(ui.toast.as_ref().unwrap().0.starts_with("Saved setup1.txt"));
        ui.m.tscale = 14;
        ui.script_do("Recall setup").unwrap();
        assert_eq!(ui.m.tscale, 12);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn gpu_timing_output_appends_and_reports_open_and_write_errors() {
        let root = std::env::temp_dir().join(format!("oos-gpu-times-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        let path = root.join("times.txt");
        append_gpu_times(&path, 1.25, 6.789).unwrap();
        append_gpu_times(&path, 2.0, 3.0).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "1.250 6.789\n2.000 3.000\n");
        let error = append_gpu_times(&root, 0.0, 0.0).unwrap_err();
        assert!(error.contains("write GPU times"), "{}", error);
        #[cfg(target_os = "linux")]
        assert!(append_gpu_times(std::path::Path::new("/dev/full"), 0.0, 0.0).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn script_size_diagnostic_stays_with_the_runner() {
        let mut ui = Ui::for_tests(crate::wave::Source::Demo { t: 0.0 });
        let error = configure(&mut ui, "script.txt", &["--size".into(), "broken".into()]).err().unwrap();
        assert_eq!(error, "--size broken: give WIDTHxHEIGHT, at least 1024x600");
    }

    #[test]
    fn script_steps_capture_names_and_quit_keep_their_meaning() {
        let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../target/host-script").join(format!(
            "runner-{}-{}",
            std::process::id(),
            nonce
        ));
        std::fs::create_dir_all(&root).unwrap();
        let (script, ppm, png, late) =
            (root.join("script.txt"), root.join("one.ppm"), root.join("two.png"), root.join("late.ppm"));
        std::fs::write(
            &script,
            format!("frames 2\nshot {}\nshot {}\nquit\nshot {}\n", ppm.display(), png.display(), late.display()),
        )
        .unwrap();
        let mut ui = Ui::for_tests(crate::wave::Source::Demo { t: 0.0 });
        run_script(&mut ui, script.to_str().unwrap(), None, None, false).unwrap();
        assert!((ui.now - 5.0 / 30.0).abs() < 1e-12, "initial step + frames + each shot");
        assert!(std::fs::read(ppm).unwrap().starts_with(b"P6\n1024 600\n255\n"));
        assert!(std::fs::read(png).unwrap().starts_with(b"\x89PNG\r\n\x1a\n"));
        assert!(!late.exists());
        assert!(!root.join("two.png.meta").exists(), "ordinary scripts keep their capture outputs unchanged");
        assert!(run_script(&mut ui, root.join("missing").to_str().unwrap(), None, None, false).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}
