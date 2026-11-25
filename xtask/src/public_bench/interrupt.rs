// SPDX-License-Identifier: GPL-2.0-or-later

use std::{
    env, io,
    os::unix::process::CommandExt,
    process::Command,
    sync::mpsc::{self, Receiver, RecvTimeoutError, TryRecvError},
    thread,
    time::{Duration, Instant},
};

use super::{Clock, Error};

pub(super) const WORKER: &str = "--benchmark-worker";

pub(super) fn supervise(args: &[&str]) -> Error {
    let result = match env::current_exe() {
        Ok(executable) => Command::new("python3")
            .args(["-I", "-c", include_str!("interrupt.py")])
            .arg(executable)
            .args(["public-bench", WORKER])
            .args(args)
            .exec(),
        Err(error) => error,
    };
    Error::InterruptControl(result)
}

#[derive(Debug)]
pub(super) struct HostClock {
    started: Instant,
    lifeline: Receiver<()>,
}

impl HostClock {
    pub(super) fn start() -> Result<Self, Error> {
        let (keep_alive, lifeline) = mpsc::channel();
        thread::Builder::new()
            .name("public-bench-interrupt".into())
            .spawn(move || {
                // The supervisor keeps the pipe open until it receives a termination signal.
                use io::Read;
                let _ = io::stdin().read(&mut [0]);
                drop(keep_alive);
            })
            .map_err(Error::InterruptControl)?;
        Ok(Self { started: Instant::now(), lifeline })
    }
}

impl Clock for HostClock {
    fn now(&self) -> Duration {
        self.started.elapsed()
    }

    fn check(&self) -> Result<(), Error> {
        match self.lifeline.try_recv() {
            Err(TryRecvError::Empty) => Ok(()),
            _ => Err(Error::Interrupted),
        }
    }

    fn wait(&self, duration: Duration) -> Result<(), Error> {
        match self.lifeline.recv_timeout(duration) {
            Err(RecvTimeoutError::Timeout) => Ok(()),
            _ => Err(Error::Interrupted),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        io::{BufRead, BufReader},
        process::{Command, Stdio},
    };

    #[test]
    fn supervisor_waits_for_cleanup_after_sigint() {
        let worker = "import sys; print('ready', flush=True); sys.stdin.read(); print('restored', flush=True)";
        let mut supervisor = Command::new("python3")
            .args(["-I", "-c", include_str!("interrupt.py"), "python3", "-I", "-c", worker])
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut output = BufReader::new(supervisor.stdout.take().unwrap()).lines();
        assert_eq!(output.next().unwrap().unwrap(), "ready");
        let status = Command::new("kill").args(["-s", "INT", &supervisor.id().to_string()]).status().unwrap();
        assert!(status.success());
        assert_eq!(output.next().unwrap().unwrap(), "restored");
        assert_eq!(supervisor.wait().unwrap().code(), Some(130));
    }
}
