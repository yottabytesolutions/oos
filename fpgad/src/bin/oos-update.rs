// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! oos-update: signed OOS updates with rollback (oos_hal::update, docs/update.md).
//!
//! On the unit:
//!   oos-update status [--quick]            what recovery and the rollback slot hold, trial state, boot mode
//!                                          (--quick: headers only, no SHA-256 over the images)
//!   oos-update install FILE.oosupd [--execute] [--downgrade] [--force] [--any-model] [--no-kexec | --new-dt]
//!   oos-update leave                     forget tracked builds before switching firmware
//!                                          check the signature and the image (dry run without --execute), then start
//!                                          the new image by kexec while recovery stays intact; that start
//!                                          installs it (rollback slot, recovery, read back, OOS armed),
//!                                          then restarts automatically for the trial. --no-kexec, --new-dt and
//!                                          packages without 'trial kexec' write it directly;
//!                                          it starts on trial at the next power-on
//!   oos-update kexec-check FILE.oosupd     load the package's kernel and initramfs for a kexec start and unload them
//!                                          again: nothing written, no restart
//!   oos-update sd-mode [hs|sdr104] [--execute]  restage the selected mode or sd.mode from system.conf, on trial
//!   oos-update card-check                  record the running boot card mode for diagnostics
//!   oos-update rollback [--execute]        write the rollback image back to recovery and arm OOS
//!   oos-update gel-install FILE.oosupd --save DIR [--any-model] [--reinstall] [--execute]
//!                                          first install from the running stock system, rescue set in DIR; again on
//!                                          a card with a valid OOS image it only selects OOS (--reinstall: writes it)
//!                                          --any-model: also on a model whose hardware variant OOS has not run on
//!                                          yet (hal target `variants`; docs/install-usb.md 4)
//!   oos-update confirm                     end the trial by hand (the watcher normally does it); only once the image
//!                                          on trial runs
//!   oos-update arm                         select OOS for the next power-on: only a valid image in recovery, never
//!                                          while an update job runs (oos-boot-oos-again)
//!   oos-update mode oneshot|persist        oneshot (default): every power-on after an OOS start boots stock;
//!                                          persist: OOS arms itself again after every confirmed start
//!   oos-update boot                        rcS, early: count the start of a trial image; a trial image that starts
//!                                          a second time without confirmation is rolled back at once
//!   oos-update watch [--status FILE]       rcS, background: judge the start (oos-ui stable 20 s and start-up
//!                                          finished = good; 3 oos-ui crashes or no end in 180 s = bad), then confirm
//!                                          (+ arm in persist mode) or roll back; message in /tmp/oos-update.notice
//! Anywhere:
//!   oos-update verify FILE.oosupd [--keys FILE]   signature and image check, no card access
//!   oos-update keygen --out FILE          new signing key outside the repository (mode 0600),
//!                                          prints the public key line for oos/keys/update.pub
//!   oos-update pack --image IMG --build ID [--kernel REL] [--models LIST] [--key FILE] --out FILE.oosupd
//!                                          or select a key with OOS_SIGNING_KEY; no automatic key discovery
//!                                          --untrusted-ok permits a key not trusted by this build
//!
//! Common options: --card /dev/mmcblk0, --state /oos/update/state, --keys FILE (extra trusted keys: verify, and
//! install on a card file for tests; refused for the unit's card).

use oos_hal::update::{self, card, hash::hex, kexec, pkg, State};
use std::path::{Path, PathBuf};

const NOTICE: &str = "/tmp/oos-update.notice";

use oos_update::args::Args;

/// `println!` for this tool's output. A failed write to stdout never ends or panics the process: a write sequence
/// (install, rollback, mode) must not stop between two steps because the reader went away (`oos-update status |
/// head -1`). After the first failure stdout counts as closed and later output is dropped; an error other than a
/// closed pipe is reported once on stderr. The exit status stays the command's.
macro_rules! out {
    ($($t:tt)*) => {
        put(&mut std::io::stdout().lock(), &STDOUT_CLOSED, format_args!($($t)*))
    };
}

static STDOUT_CLOSED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn put(w: &mut dyn std::io::Write, closed: &std::sync::atomic::AtomicBool, a: std::fmt::Arguments) {
    use std::sync::atomic::Ordering;
    if closed.load(Ordering::Relaxed) {
        return;
    }
    let r = w.write_fmt(a).and_then(|_| w.write_all(b"\n"));
    if let Err(e) = r {
        closed.store(true, Ordering::Relaxed);
        if e.kind() != std::io::ErrorKind::BrokenPipe {
            eprintln!("oos-update: stdout: {} (further output dropped)", e);
        }
    }
}

mod oos_update;

#[cfg(target_os = "linux")]
use oos_update::watch::run as watch;

use oos_update::{
    messages,
    sign::{keygen, keys, pack},
};

fn die(msg: &str) -> ! {
    eprintln!("oos-update: {}", msg);
    std::process::exit(1)
}

/// Lines also to the kernel log (and the panel console, which shows when oos-ui is down).
fn tell(msg: &str) {
    out!("{}", msg);
    // one write per line: every write to /dev/kmsg is a record of its own
    let line = format!("oos-update: {}\n", msg);
    for dev in ["/dev/kmsg", "/dev/tty1"] {
        if Path::new(dev).exists() {
            let _ = std::fs::OpenOptions::new().append(true).open(dev).and_then(|mut f| {
                use std::io::Write;
                f.write_all(line.as_bytes())
            });
        }
    }
}

fn notice(msg: &str) {
    if let Err(error) = std::fs::write(NOTICE, format!("{msg}\n")) {
        tell(&format!("writing the update notice: {error}"));
    }
    tell(msg);
}

/// The SD card as the stock system names it.
fn stock_card() -> Option<String> {
    ["mmcblk0", "mmcblk1"].iter().find_map(|n| {
        let dev = std::fs::canonicalize(format!("/sys/block/{}/device", n)).ok()?;
        dev.to_string_lossy().contains(oos_hal::target::ACTIVE.card_controller).then(|| format!("/dev/block/{}", n))
    })
}

/// First install from the stock system (docs/install-usb.md); `shell/do_extract.sh` of the update file runs this.
fn gel_install(a: &Args) {
    let f = a
        .rest
        .get(1)
        .unwrap_or_else(|| die("gel-install FILE.oosupd --save DIR [--any-model] [--reinstall] [--execute]"));
    let save = a.val("--save").unwrap_or_else(|| die("gel-install needs --save DIR on the USB stick"));
    let p = read_file(f);
    let opt = update::first::FirstOpt {
        execute: a.flag("--execute"),
        any_model: a.flag("--any-model"),
        reinstall: a.flag("--reinstall"),
    };
    let path = a
        .val("--card")
        .map(str::to_string)
        .or_else(stock_card)
        .unwrap_or_else(|| die(&format!("no SD card on {}", oos_hal::target::ACTIVE.card_controller)));
    out!("card {}", path);
    let vendor = std::fs::read(a.val("--vendor").unwrap_or("/rigol/data/vendor.bin")).ok();
    let mut c = card::open_checked(Path::new(&path), opt.execute).unwrap_or_else(|e| die(&e));
    let keys = keys(a, c.is_test_file().unwrap_or_else(|e| die(&e.to_string())));
    let r = update::first::first_install(&mut c, &p, &keys, Path::new(save), vendor.as_deref(), &opt, &mut |l| {
        out!("{}", l)
    });
    if let Err(e) = r {
        die(&e);
    }
}

fn state_path(a: &Args) -> PathBuf {
    PathBuf::from(a.val("--state").unwrap_or(update::STATE_PATH))
}

/// The state file lives on /oos: refuse to work without it (a fresh state would forget the rollback image).
fn need_data(a: &Args) {
    if a.val("--state").is_some() {
        return;
    }
    if let Ok(m) = std::fs::read_to_string("/proc/mounts") {
        if !oos_hal::system::oos_mounted(&m) {
            die("/oos (OOS data partition) is not mounted: the update state lives there (oos-mount-data)");
        }
    }
}

fn read_file(p: &str) -> Vec<u8> {
    std::fs::read(p).unwrap_or_else(|e| die(&format!("{}: {}", p, e)))
}

fn main() {
    let a = Args::parse();
    match a.command() {
        "status" => oos_update::startup::status(&a),
        "leave" => oos_update::startup::leave(&a),
        "boot" => oos_update::startup::boot(&a),
        "verify" => {
            let f = a.rest.get(1).unwrap_or_else(|| die("verify FILE"));
            let p = read_file(f);
            match pkg::verify(&p, &keys(&a, true)) {
                Ok((h, img)) => {
                    let m = &h.manifest;
                    out!("{}: signature ok (key {}), build {}, {} bytes", f, hex(&m.key), m.build, m.size);
                    match card::check_image(img) {
                        Ok(i) => out!("image '{}', loader id {} ok", i.name, hex(&i.id)),
                        Err(e) => die(&format!("image: {}", e)),
                    }
                }
                Err(e) => die(&e),
            }
        }
        "install" => install(&a),
        "gel-install" => gel_install(&a),
        "kexec-check" => kexec_check(&a),
        "card-check" => oos_update::card_check::run(),
        "sd-mode" | "rollback" | "storage-write" | "storage-data" => oos_update::storage::run(&a),
        "confirm" => {
            need_data(&a);
            let running = std::fs::read_to_string("/etc/oos-build").ok();
            match update::confirm_running(&state_path(&a), None, running.as_deref()) {
                Ok(true) => out!("trial ended: the image in recovery is confirmed"),
                Ok(false) => out!("no image on trial"),
                Err(e) => die(&e),
            }
        }
        "arm" => {
            need_data(&a);
            let mut c = a.open_card(true).unwrap_or_else(|e| die(&e));
            match update::arm_by_hand(&mut c, &state_path(&a)) {
                Ok(true) => out!("next boot: OOS"),
                Ok(false) => out!("OOS was already selected"),
                Err(e) => die(&e),
            }
        }
        "mode" => oos_update::startup::mode(&a),
        "watch" => watch(&a),
        "keygen" => keygen(&a),
        "pack" => pack(&a),
        cmd => die(&format!("unknown command '{}' (see the head of fpgad/src/bin/oos-update.rs)", cmd)),
    }
}

fn install(a: &Args) {
    let f = a.rest.get(1).unwrap_or_else(|| die("install FILE.oosupd [--execute] [--no-kexec | --new-dt]"));
    need_data(a);
    let p = read_file(f);
    let mut c = a.open_card(a.flag("--execute")).unwrap_or_else(|e| die(&e));
    let test_card = c.is_test_file().unwrap_or_else(|e| die(&e.to_string()));
    let running_model = a.running_model(test_card);
    let opt = a.install_opt(&running_model);
    let keys = keys(a, test_card);
    let sp = state_path(a);
    let announced = pkg::verify_header(&p, &keys).is_ok_and(|h| kexec::announced(&h.manifest));
    // a card file on the build machine cannot be started
    let by_kexec = !test_card && !a.flag("--no-kexec") && !a.flag("--new-dt") && announced;
    if !by_kexec {
        if !test_card && !a.flag("--no-kexec") && !a.flag("--new-dt") {
            out!("the package does not announce a kexec receiver (an older build): written directly, without the kexec trial start");
        }
        if a.flag("--new-dt") {
            out!("--new-dt: written directly; a kernel or device tree that hangs before rcS starts again at every power-on");
        }
        match update::install_direct(&mut c, &sp, &p, &keys, &opt) {
            Ok(r) => {
                r.lines.iter().for_each(|l| out!("{}", l));
                if r.written && c.boot_magic().ok() == Some(*b"OOSK") {
                    out!("{}", update::text::DIRECT_START);
                }
            }
            Err(e) => die(&e),
        }
        return;
    }
    let booted = std::fs::read(kexec::BOOTED_DT).ok();
    let running = update::running_build();
    let has_tool = Path::new(kexec::TOOL)
        .try_exists()
        .unwrap_or_else(|e| die(&format!("checking for kexec-tools ({}): {e}", kexec::TOOL)));
    let running_tree = has_tool.then(|| {
        std::fs::read(kexec::RUNNING_TREE)
            .unwrap_or_else(|e| die(&format!("reading the running device tree ({}): {e}", kexec::RUNNING_TREE)))
    });
    let trees = kexec::BootTrees {
        booted: booted.as_deref(),
        running_build: running.as_deref(),
        live: running_tree.as_deref(),
    };
    match update::kexec_stage(&mut c, &sp, &p, &keys, &opt, trees) {
        Err(e) if e.contains("device tree") && !e.contains("--new-dt") => die(&format!(
            "{}. Try the image on a card you can take out; then --new-dt writes it directly, without the kexec trial start",
            e
        )),
        Err(e) => die(&e),
        Ok((r, None)) => r.lines.iter().for_each(|l| out!("{}", l)),
        Ok((r, Some(s))) => {
            r.lines.iter().for_each(|l| out!("{}", l));
            drop(p);
            start_kexec(&sp, s);
        }
    }
}

/// Resume the image supervisor after kexec undo only if its original loop has already exited.
/// A probe error is not evidence that DRM has no owner.
#[cfg(any(target_os = "linux", test))]
const UI_LOOP: &str = "pgrep -x oos-ui-loop >/dev/null; rc=$?; [ \"$rc\" -ne 1 ] || exec /bin/oos-ui-loop";

/// What was stopped for the kexec start, to put back after a failure.
#[cfg(target_os = "linux")]
struct Quiet {
    links: Vec<String>,
    routes: Vec<String>,
    display: bool,
}

#[cfg(target_os = "linux")]
const DRM: &str = "/sys/bus/platform/drivers/rockchip-drm";

/// Devices that write to memory on their own stop before the kexec start; the kernel does not stop them all. The
/// Ethernet controller has no shutdown hook: its receive DMA would go on writing packets into buffers of this kernel,
/// which is memory of the next one by then (seen: the package arrived damaged). The display keeps its IOMMU on, which
/// the next kernel then cannot take over (seen: a dark screen).
#[cfg(target_os = "linux")]
fn quiesce() -> Quiet {
    let out = |args: &[&str]| {
        std::process::Command::new(args[0])
            .args(&args[1..])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
            .unwrap_or_default()
    };
    let routes: Vec<String> =
        out(&["ip", "-4", "route", "show"]).lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect();
    let mut links = Vec::new();
    if let Ok(d) = std::fs::read_dir("/sys/class/net") {
        for e in d.flatten() {
            let n = e.file_name().to_string_lossy().into_owned();
            let up = std::fs::read_to_string(format!("/sys/class/net/{}/flags", n))
                .ok()
                .and_then(|f| u32::from_str_radix(f.trim().trim_start_matches("0x"), 16).ok())
                .is_some_and(|f| f & 1 != 0);
            if n != "lo"
                && up
                && std::process::Command::new("ip")
                    .args(["link", "set", "dev", &n, "down"])
                    .status()
                    .is_ok_and(|x| x.success())
            {
                links.push(n);
            }
        }
    }
    // the display subsystem lets go of the VOPs and their IOMMUs while it still owns them
    let display = Path::new(&format!("{}/display-subsystem", DRM)).exists()
        && std::fs::write(format!("{}/unbind", DRM), "display-subsystem").is_ok();
    tell(&format!(
        "stopped for the kexec start: links {:?}, display {}",
        links,
        if display { "unbound" } else { "left" }
    ));
    Quiet { links, routes, display }
}

#[cfg(target_os = "linux")]
fn restore(q: &Quiet) {
    if q.display {
        kexec::undo::warn("bind display", std::fs::write(format!("{}/bind", DRM), "display-subsystem"));
    }
    for n in &q.links {
        kexec::undo::ip("set link up", &["link", "set", "dev", n, "up"]);
    }
    // a link that went down took its routes along
    for r in &q.routes {
        let mut a = vec!["route", "replace"];
        a.extend(r.split_whitespace());
        kexec::undo::ip("restore route", &a);
    }
}

#[cfg(target_os = "linux")]
use oos_update::recovery::start_kexec;

#[cfg(not(target_os = "linux"))]
fn start_kexec(sp: &Path, s: update::Staged) {
    let _ = update::kexec_clear(sp, "kexec runs on the unit only");
    die(&format!("kexec start of build {}: on the unit only", s.build));
}

/// The kernel takes the image for a kexec start (CPUs can go offline, the Image header is accepted).
#[cfg(target_os = "linux")]
fn kexec_check(a: &Args) {
    let f = a.rest.get(1).unwrap_or_else(|| die("kexec-check FILE.oosupd"));
    let p = read_file(f);
    let keys = update::trusted_keys();
    let (h, img) = pkg::verify(&p, &keys).unwrap_or_else(|e| die(&e));
    let (k, rd, dt) = kexec::parts(img).unwrap_or_else(|e| die(&e));
    let key = keys.iter().find(|k| pkg::key_id(k) == h.manifest.key).unwrap_or_else(|| die("no key"));
    let flags = kexec::Flags::default();
    let (source, initrd) = if kexec::use_staged(&h.manifest, key).unwrap_or_else(|e| die(&e)) {
        let handoff = kexec::StagedInitrd { ramdisk: rd, header: &p[..h.data_offset], key, tree: dt };
        (kexec::Receive::Staged(flags), kexec::staged_initrd(handoff))
    } else {
        (kexec::Receive::Legacy(flags), kexec::initrd(rd, &p, key, dt))
    };
    let cmd = source.cmdline(&kexec::running_cmdline());
    if kexec::loaded() {
        die("a kexec kernel is loaded already: left alone");
    }
    let t = std::time::Instant::now();
    kexec::load(k, &initrd, &cmd).unwrap_or_else(|e| die(&e));
    let ok = kexec::loaded();
    let ms = t.elapsed().as_millis();
    kexec::unload().unwrap_or_else(|e| die(&e));
    let tree = match std::fs::read(kexec::BOOTED_DT) {
        Ok(b) if b == dt => "the same as the running one",
        Ok(_) if Path::new(kexec::TOOL).exists() => {
            "DIFFERS from the running one (the kexec start tries the new one, with kexec-tools)"
        }
        Ok(_) => "DIFFERS from the running one (install needs --new-dt)",
        Err(_) => "running one unknown",
    };
    out!(
        "build {}: kernel {} bytes, initramfs {} bytes loaded in {} ms ({}), unloaded again; announced kexec receiver: {}; device tree {}",
        update::short(&h.manifest.build),
        k.len(),
        initrd.len(),
        ms,
        if ok { "kexec_loaded 1" } else { "kexec_loaded NOT 1" },
        kexec::announced(&h.manifest),
        tree
    );
    out!("command line: {}", cmd);
    if !ok || kexec::loaded() {
        die("unexpected kexec_loaded state");
    }
}

#[cfg(not(target_os = "linux"))]
fn kexec_check(_: &Args) {
    die("kexec-check runs on the unit only");
}

/// The oos-ui process: the one followed so far while it lives (a child that oos-ui forks is "oos-ui" until its exec),
/// else the oldest one.
#[cfg(target_os = "linux")]
fn ui_pid(keep: Option<u32>) -> Option<u32> {
    let is_ui = |pid: u32| std::fs::read_to_string(format!("/proc/{}/comm", pid)).is_ok_and(|c| c.trim() == "oos-ui");
    if let Some(k) = keep.filter(|&k| is_ui(k)) {
        return Some(k);
    }
    let mut best: Option<u32> = None;
    for e in std::fs::read_dir("/proc").ok()?.flatten() {
        let Ok(pid) = e.file_name().to_string_lossy().parse::<u32>() else { continue };
        if is_ui(pid) {
            best = Some(best.map_or(pid, |b| b.min(pid)));
        }
    }
    best
}

/// The start-up watcher (`oos-update watch`) of this start: stopped before a kexec start (called with the lock held).
#[cfg(target_os = "linux")]
fn stop_watcher() {
    let me = std::process::id();
    let Ok(d) = std::fs::read_dir("/proc") else { return };
    for e in d.flatten() {
        let Ok(pid) = e.file_name().to_string_lossy().parse::<u32>() else { continue };
        if pid == me {
            continue;
        }
        let cmd = std::fs::read(format!("/proc/{}/cmdline", pid)).unwrap_or_default();
        let args: Vec<&[u8]> = cmd.split(|&b| b == 0).collect();
        let is_update = args.first().is_some_and(|a0| a0.ends_with(b"oos-update"));
        if is_update && args.get(1) == Some(&&b"watch"[..]) {
            unsafe { libc::kill(pid as i32, libc::SIGKILL) };
        }
    }
}

/// Whether the kexec start that just installed `build` restarts the scope for its trial start: only with the image
/// written (`OOSK` armed, so the restart cannot drop to stock), the build on trial, and no restart for it before (a
/// start that comes back here again does not loop).
fn may_restart(magic: Option<[u8; 4]>, on_trial: bool, mark: Option<&str>, build: &str) -> Result<(), String> {
    if magic != Some(*b"OOSK") {
        return Err("the boot magic is not OOSK".into());
    }
    if !on_trial {
        return Err("the build is not on trial".into());
    }
    if mark.is_some_and(|m| m.trim() == build.trim()) {
        return Err("this kexec start already restarted once for the build".into());
    }
    Ok(())
}

/// After the install in a kexec start: restart into the trial start, as a power-cycle would (the kexec session runs on
/// the old build's device tree and interrupt tables: with MSI the acquisition got no data). Refused: the notice asks
/// for a power-cycle.
fn restart_for_trial(a: &Args, p: &Path) {
    let build = update::running_build().unwrap_or_default();
    let magic = a.open_card(false).unwrap_or_else(|e| die(&e)).boot_magic().ok();
    let st = State::load(p).unwrap_or_default();
    let on_trial = st.trial.is_some() && st.current.as_ref().is_some_and(|c| c.build.trim() == build.trim());
    if let Err(e) =
        may_restart(magic, on_trial, std::fs::read_to_string(update::restart_mark(p)).ok().as_deref(), &build)
    {
        notice(&messages::no_restart(&build, &e, magic == Some(*b"OOSK") && on_trial));
        return;
    }
    let mark = update::restart_mark(p);
    if let Err(e) = update::write_restart_mark(&mark, &build) {
        notice(&messages::no_restart(&build, &format!("mark not written: {}", e), true));
        return;
    }
    oos_update::card_check::before_restart();
    tell(&format!(
        "build {} installed and armed (OOSK, on trial): restarting automatically for the trial start",
        update::short(&build)
    ));
    let _ = std::fs::write("/dev/tty1", "\nUpdate installed: restarting for the trial start.\n");
    restart();
}

#[cfg(target_os = "linux")]
fn restart() {
    use oos_hal::system::DataShutdown;
    use oos_update::recovery;

    // Leave the notice on the panel before the trial restart. rcS has not started the UI yet.
    std::thread::sleep(std::time::Duration::from_secs(3));
    recovery::restart(
        DataShutdown::prepare,
        || {
            // SAFETY: reboot takes a scalar command. The caller verified OOSK and the installed trial
            // before entering this path, so a successful restart boots the trial via the normal loader.
            let rc = unsafe { libc::reboot(libc::LINUX_REBOOT_CMD_RESTART) };
            format!("reboot returned {rc}: {}", std::io::Error::last_os_error())
        },
        DataShutdown::restore,
        |why| notice(&messages::no_restart(&update::running_build().unwrap_or_default(), why, true)),
    );
}

#[cfg(not(target_os = "linux"))]
fn restart() {
    tell("restart: on the unit only");
}

#[cfg(not(target_os = "linux"))]
fn watch(_: &Args) {
    die("watch runs on the unit only");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[test]
    fn kexec_undo_uses_supervisor_only_when_no_loop_remains() {
        let root = oos_update::test_support::TempDir::new();
        use std::os::unix::fs::PermissionsExt;
        let probe = root.path().join("pgrep");
        std::fs::write(&probe, "#!/bin/sh\n[ \"$*\" = '-x oos-ui-loop' ] || exit 2\nexit \"$PROBE_STATUS\"\n").unwrap();
        std::fs::set_permissions(&probe, std::fs::Permissions::from_mode(0o755)).unwrap();
        let script = UI_LOOP.replace("/bin/oos-ui-loop", "echo supervisor");
        for (status, wanted) in [(0, ""), (1, "supervisor"), (2, "")] {
            let out = std::process::Command::new("sh")
                .args(["-c", &script])
                .env("PATH", format!("{}:{}", root.path().display(), std::env::var("PATH").unwrap()))
                .env("PROBE_STATUS", status.to_string())
                .output()
                .unwrap();
            assert_eq!(String::from_utf8(out.stdout).unwrap().trim(), wanted, "probe {}", status);
        }
    }

    // Break it catches: a restart with KRNL armed (the trial start would be stock), of a build that is not on trial,
    // or a second restart from a kexec start that came back.
    #[test]
    fn a_kexec_install_restarts_only_armed_on_trial_and_once() {
        let b = "20261114-120000";
        assert!(may_restart(Some(*b"OOSK"), true, None, b).is_ok());
        assert!(may_restart(Some(*b"OOSK"), true, Some("20261101-000000\n"), b).is_ok());
        assert!(may_restart(Some(*b"KRNL"), true, None, b).unwrap_err().contains("OOSK"));
        assert!(may_restart(None, true, None, b).is_err());
        assert!(may_restart(Some(*b"OOSK"), false, None, b).unwrap_err().contains("trial"));
        assert!(may_restart(Some(*b"OOSK"), true, Some("20261114-120000\n"), b).unwrap_err().contains("already"));
    }

    #[test]
    fn the_restart_mark_sits_beside_the_state() {
        assert_eq!(update::restart_mark(Path::new("/oos/update/state")), Path::new("/oos/update/kexec-restart"));
    }

    /// Counts write calls; fails every one with `fail`, or none.
    struct Pipe {
        calls: usize,
        fail: Option<std::io::ErrorKind>,
        got: Vec<u8>,
    }

    impl std::io::Write for Pipe {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            self.calls += 1;
            match self.fail {
                Some(k) => Err(k.into()),
                None => {
                    self.got.extend_from_slice(b);
                    Ok(b.len())
                }
            }
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    // Break it catches: a closed reader ending or panicking the process mid-command (put returns, so the command
    // continues), later output still written to the dead pipe, normal output changed.
    #[test]
    fn closed_pipe_drops_output_and_continues() {
        let closed = AtomicBool::new(false);
        let mut p = Pipe { calls: 0, fail: Some(std::io::ErrorKind::BrokenPipe), got: Vec::new() };
        put(&mut p, &closed, format_args!("state {}", 1));
        assert!(closed.load(Ordering::Relaxed));
        let n = p.calls;
        put(&mut p, &closed, format_args!("trial {}", "none"));
        assert_eq!(p.calls, n, "nothing written after the pipe closed");

        let open = AtomicBool::new(false);
        let mut ok = Pipe { calls: 0, fail: None, got: Vec::new() };
        put(&mut ok, &open, format_args!("trial {}", "none"));
        put(&mut ok, &open, format_args!("mode {}", "persist"));
        assert_eq!((ok.got.as_slice(), open.load(Ordering::Relaxed)), (&b"trial none\nmode persist\n"[..], false));

        let other = AtomicBool::new(false);
        put(
            &mut Pipe { calls: 0, fail: Some(std::io::ErrorKind::PermissionDenied), got: Vec::new() },
            &other,
            format_args!("x"),
        );
        assert!(other.load(Ordering::Relaxed), "any write error closes stdout for the rest of the run");
    }
}
