// SPDX-License-Identifier: GPL-2.0-or-later
//! Four outstanding operations, with setup text at most 64 KiB plus a 4 KiB path. Credits include unconsumed results.
//! Automatic observations retain only the latest candidate and never displace an explicit save.
use super::*;
use crate::ui::lifecycle::worker::Worker as Jobs;
use std::sync::mpsc::{TryRecvError, TrySendError};

const ENTRIES: usize = 4;
const TEXT_BYTES: usize = 64 * 1024;
const PATH_BYTES: usize = 4096;

pub(crate) enum Done {
    Save(Box<Path>, io::Result<()>),
    Recall(Box<Path>, io::Result<String>),
    Automatic(io::Result<()>),
}

struct Completion {
    done: Done,
    promoted: Option<(Box<str>, Duration)>,
}

struct Candidate {
    text: Box<str>,
    since: Duration,
    promoted: bool,
}

enum Mode {
    Observe,
    Finish(bool),
}

enum Job {
    Save(Box<Path>, Box<str>),
    Recall(Box<Path>),
    Automatic(Box<str>, Duration, Duration, Mode),
    Cleanup(bool),
}

pub(crate) struct Worker {
    jobs: Jobs<Job, Completion>,
    candidate: Option<Candidate>,
}

impl Worker {
    pub(crate) fn spawn<F: Files + Send + 'static>(mut fs: F, mut startup: Option<Startup<F>>) -> io::Result<Self> {
        let jobs = Jobs::bounded("storage", ENTRIES, move |job| {
            let mut promoted = None;
            let done = match job {
                Job::Save(path, text) => {
                    let fs = startup.as_mut().map_or(&mut fs, |s| &mut s.fs);
                    let result = fs.prepare(&path).and_then(|()| durable::replace(fs, &path, &text));
                    Done::Save(path, result)
                }
                Job::Recall(path) => {
                    let fs = startup.as_ref().map_or(&fs, |s| &s.fs);
                    let result = fs.read_limited(&path, TEXT_BYTES);
                    Done::Recall(path, result)
                }
                Job::Automatic(text, since, now, mode) => {
                    let result = startup.as_mut().map_or(Ok(()), |s| {
                        s.candidate = Some((text.to_string(), since));
                        match mode {
                            Mode::Finish(true) => s.power_off(text.to_string(), now),
                            Mode::Finish(false) => s.clean(text.to_string(), now, false),
                            Mode::Observe => s.observe(text.to_string(), now),
                        }
                    });
                    if result.is_ok()
                        && startup
                            .as_ref()
                            .is_none_or(|s| s.disabled || (s.stable && s.persisted.as_deref() == Some(&text)))
                    {
                        promoted = Some((text, since));
                    }
                    Done::Automatic(result)
                }
                Job::Cleanup(power) => Done::Automatic(startup.as_mut().map_or(Ok(()), |s| {
                    let cleaned = s.clear_markers();
                    if power {
                        cleaned.and(s.stop_supervisor())
                    } else {
                        cleaned
                    }
                })),
            };
            Completion { done, promoted }
        })?;
        Ok(Self { jobs, candidate: None })
    }

    fn submit(&mut self, job: Job) -> io::Result<()> {
        self.jobs.submit(job).map_err(|error| match error {
            TrySendError::Full(_) => io::Error::new(io::ErrorKind::WouldBlock, "setup storage queue full"),
            TrySendError::Disconnected(_) => io::Error::other("setup storage worker disconnected"),
        })
    }

    pub(crate) fn save(&mut self, path: PathBuf, text: String) -> io::Result<()> {
        Self::check_size(text.len(), TEXT_BYTES)?;
        Self::check_size(path.as_os_str().len(), PATH_BYTES)?;
        self.submit(Job::Save(path.into_boxed_path(), text.into_boxed_str()))
    }

    pub(crate) fn busy(&self) -> bool {
        self.jobs.busy()
    }

    pub(crate) fn recall(&mut self, path: PathBuf) -> io::Result<()> {
        Self::check_size(path.as_os_str().len(), PATH_BYTES)?;
        self.submit(Job::Recall(path.into_boxed_path()))
    }

    fn check_size(bytes: usize, limit: usize) -> io::Result<()> {
        if bytes > limit {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, format!("setup save exceeds {limit} bytes")));
        }
        Ok(())
    }

    pub(crate) fn observe(&mut self, text: String, now: Duration) -> io::Result<()> {
        if let Err(error) = Self::check_size(text.len(), TEXT_BYTES) {
            self.candidate = None;
            return Err(error);
        }
        if self.candidate.as_ref().is_none_or(|old| *old.text != text) {
            self.candidate = Some(Candidate { text: text.into_boxed_str(), since: now, promoted: false });
        }
        if let Some(candidate) = self
            .candidate
            .as_ref()
            .filter(|c| !c.promoted && !self.jobs.busy() && now.saturating_sub(c.since) >= STABLE)
        {
            self.submit(Job::Automatic(candidate.text.clone(), candidate.since, now, Mode::Observe))?;
        }
        Ok(())
    }

    pub(crate) fn poll(&mut self, wait: bool) -> Option<Done> {
        if !self.jobs.busy() {
            return None;
        }
        let result =
            if wait { self.jobs.drain().next()?.map_err(|_| TryRecvError::Disconnected) } else { self.jobs.poll() };
        match result {
            Ok(Completion { done, promoted }) => {
                if let (Some(candidate), Some((text, since))) = (&mut self.candidate, promoted) {
                    if candidate.text == text && candidate.since == since {
                        candidate.promoted = true;
                    }
                }
                Some(done)
            }
            Err(TryRecvError::Empty) => None,
            Err(error) => Some(Done::Automatic(Err(io::Error::other(error)))),
        }
    }

    #[cfg(test)]
    pub(crate) fn wait_ready(&self) {
        self.jobs.wait_ready("setup storage completion");
    }

    pub(crate) fn flush(&mut self) -> io::Result<()> {
        // Consume all four possible completions before reporting the first error.
        let completed: Vec<_> = std::iter::from_fn(|| self.poll(true)).collect();
        completed.into_iter().try_for_each(|done| match done {
            Done::Save(_, result) | Done::Automatic(result) => result,
            Done::Recall(_, result) => result.map(|_| ()),
        })
    }

    pub(crate) fn finish(&mut self, text: String, now: Duration, power: bool) -> io::Result<()> {
        let pending = self.flush();
        let size = Self::check_size(text.len(), TEXT_BYTES);
        let job = if size.is_ok() {
            let since = self.candidate.as_ref().filter(|c| *c.text == text).map_or(now, |c| c.since);
            Job::Automatic(text.into_boxed_str(), since, now, Mode::Finish(power))
        } else {
            Job::Cleanup(power)
        };
        self.submit(job)?;
        let finished = self.flush();
        pending.and(size).and(finished)
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        if let Err(error) = self.flush() {
            eprintln!("oos-ui: shutdown save: {}", error);
        }
    }
}
