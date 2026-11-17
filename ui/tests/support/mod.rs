// SPDX-License-Identifier: GPL-2.0-or-later

pub mod cases;
pub mod counting;

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub struct Inputs {
    root: PathBuf,
}

impl Inputs {
    pub fn path(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }
}

impl Drop for Inputs {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

pub fn isolated_inputs() -> Inputs {
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH).expect("test clock is after the Unix epoch").as_nanos();
    reserve_inputs(nonce)
}

fn reserve_inputs(nonce: u128) -> Inputs {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let output = Command::new("python3")
        .arg(root.join("tools/output.py"))
        .arg("target")
        .output()
        .expect("Python and tools/output.py must resolve worktree outputs");
    assert!(output.status.success(), "output target failed");
    let parent = PathBuf::from(String::from_utf8(output.stdout).expect("output helper emits a UTF-8 directory").trim())
        .join("host-script");
    std::fs::create_dir_all(&parent).expect("isolated test parent must be writable");
    for attempt in 0..1024 {
        let root = parent.join(format!("{}-{}-{}", std::process::id(), nonce, attempt));
        match std::fs::create_dir(&root) {
            Ok(()) => {
                std::fs::create_dir(root.join("store")).expect("reserved test storage must be writable");
                return Inputs { root };
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => panic!("reserve isolated test directory: {}", e),
        }
    }
    panic!("isolated test directory collision limit exceeded");
}

#[test]
fn repeated_clock_values_cannot_share_child_inputs() {
    let a = reserve_inputs(0);
    let b = reserve_inputs(0);
    assert_ne!(a.root, b.root);
    std::fs::write(a.path("kept"), "a").unwrap();
    drop(b);
    assert_eq!(std::fs::read_to_string(a.path("kept")).unwrap(), "a");
}

pub fn run_command(inputs: &Inputs, command: Command, timeout: u64) -> Output {
    run_owned(inputs, command, timeout, false)
}

pub fn run_group(inputs: &Inputs, command: Command, timeout: u64) -> Output {
    run_owned(inputs, command, timeout, true)
}

fn run_owned(inputs: &Inputs, mut command: Command, timeout: u64, group: bool) -> Output {
    let out = inputs.path("stdout.log");
    let err = inputs.path("stderr.log");
    command
        .stdout(Stdio::from(std::fs::File::create(&out).expect("isolated stdout log must be writable")))
        .stderr(Stdio::from(std::fs::File::create(&err).expect("isolated stderr log must be writable")));
    if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
        command.env("LLVM_PROFILE_FILE", profile);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        if group {
            command.process_group(0);
        }
    }
    let mut child = command.spawn().expect("Cargo supplies an executable host binary");
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().expect("the test owns the running child") {
            break status;
        }
        if start.elapsed() > Duration::from_secs(timeout) {
            eprintln!("hang guard: owned test child did not exit");
            #[cfg(unix)]
            if group {
                kill_group(child.id());
            }
            let _ = child.kill();
            break child.wait().expect("timed-out child must be reaped by its test");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    Output {
        status,
        stdout: std::fs::read(out).expect("child stdout log was created"),
        stderr: std::fs::read(err).expect("child stderr log was created"),
    }
}

/// SIGKILL to the process group `pgid`. The `--` is required: procps-ng 4.0.4 `kill` (Ubuntu 24.04) reads
/// `-KILL -12345` as `kill(-1, SIGKILL)`, which kills every process of the user, the CI runner included (#23).
#[cfg(unix)]
fn kill_group(pgid: u32) {
    match Command::new("/bin/kill").args(["-KILL", "--", &format!("-{pgid}")]).status() {
        Ok(status) if status.success() => {}
        result => eprintln!("hang guard: kill of process group {pgid} failed: {result:?}"),
    }
}

#[cfg(unix)]
#[test]
fn group_kill_reaches_grandchildren_and_nothing_else() {
    use std::io::{BufRead, Read};
    use std::os::unix::process::CommandExt;
    let (reader, writer) = std::io::pipe().expect("test pipe");
    // fd 3 holds the pipe in every member of the group. A grandchild that survives the kill reports it after 120 s
    // (hang guard); a killed group closes the pipe at once.
    let script = "exec 3<&0 0</dev/null; (sleep 120; echo survived >&3) & echo ready >&3; wait";
    let mut group = Command::new("sh");
    group.args(["-c", script]).stdin(writer).process_group(0);
    let mut leader = group.spawn().expect("sh starts");
    drop(group);
    let mut bystander = Command::new("sleep").arg("120").process_group(0).spawn().expect("sleep starts");
    let mut reader = std::io::BufReader::new(reader);
    let mut line = String::new();
    reader.read_line(&mut line).expect("group start is readable");
    assert_eq!(line, "ready\n");
    kill_group(leader.id());
    let mut rest = String::new();
    reader.read_to_string(&mut rest).expect("group output is readable");
    assert_eq!(rest, "", "a grandchild outlived the group kill");
    assert!(!leader.wait().expect("leader is reaped").success());
    assert!(bystander.try_wait().expect("bystander state is readable").is_none(), "the kill reached another group");
    bystander.kill().expect("bystander is ours");
    bystander.wait().expect("bystander is reaped");
}

#[test]
fn deadline_kills_and_reaps_the_owned_child() {
    for group in [false, true] {
        let inputs = isolated_inputs();
        let mut command = Command::new("python3");
        command.args(["-c", "import time; time.sleep(60)"]);
        let result = if group { run_group(&inputs, command, 0) } else { run_command(&inputs, command, 0) };
        assert!(!result.status.success());
    }
}
