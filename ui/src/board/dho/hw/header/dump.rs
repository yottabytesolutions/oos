// SPDX-License-Identifier: GPL-2.0-or-later

//! Opt-in, bounded DMA captures. Acquisition writes the bytes after the read clock stops.
//! The last batch owner publishes the sidecar after the placement jobs return.

use super::format::{Input, DATA_LIMIT, META_LIMIT};
use crate::board::Frame;
use crate::drawpipe::route::{RouteReport, RouteTrace};
use std::fs::{self, DirBuilder, File};
use std::io::{self, BufWriter, Write};
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const MAX_BATCHES: usize = 256;
const MAX_PER_CHANGE: usize = 32;

pub(in crate::board::dho::hw) use super::format::Input as BatchInput;

pub(in crate::board::dho::hw) struct Dump {
    root: PathBuf,
    build: String,
    next: usize,
    remaining: u64,
    per_change: usize,
    change: Option<((u64, u64), u64)>,
    left: usize,
}

impl Dump {
    #[cfg(target_os = "linux")]
    pub(in crate::board::dho::hw) fn from_switch(config: &crate::sysconf::switches::BatchDump) -> Option<Self> {
        let (root, per_change) = config.destination()?;
        let build = oos_hal::update::running_build()
            .filter(|build| valid_build(build))
            .unwrap_or_else(|| format!("oos-ui-{}", env!("CARGO_PKG_VERSION")));
        Self::open(root.to_path_buf(), per_change, build).map_err(|e| eprintln!("oos-ui batch dump: {e}")).ok()
    }

    pub(super) fn open(root: PathBuf, per_change: usize, build: String) -> io::Result<Self> {
        create_private(&root).or_else(|e| {
            use std::os::unix::fs::PermissionsExt;
            let meta = fs::symlink_metadata(&root)?;
            if e.kind() == io::ErrorKind::AlreadyExists && meta.is_dir() && meta.permissions().mode() & 0o077 == 0 {
                Ok(())
            } else {
                Err(e)
            }
        })?;
        let (mut next, mut used) = (0, 0u64);
        for slot in 0..MAX_BATCHES {
            let path = batch_path(&root, slot);
            match fs::symlink_metadata(&path) {
                Ok(meta) if meta.is_dir() => {
                    next = slot + 1;
                    for file in fs::read_dir(path)? {
                        used = used.saturating_add(file?.metadata()?.len());
                    }
                }
                Ok(_) => return Err(io::Error::new(io::ErrorKind::AlreadyExists, "invalid batch slot")),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        }
        Ok(Self {
            root,
            build,
            next,
            remaining: DATA_LIMIT.saturating_sub(used),
            per_change: per_change.clamp(1, MAX_PER_CHANGE),
            change: None,
            left: 0,
        })
    }

    #[cfg(any(target_os = "linux", test))]
    pub(in crate::board::dho::hw) fn capture_live(
        &mut self,
        pair: (u64, u64),
        input: Input<'_>,
        bytes: &[u8],
        newest: (Option<&Frame>, Option<&[u16]>),
    ) -> Option<Pending> {
        #[cfg(target_os = "linux")]
        let view_gen = crate::drawpipe::pipe().map_or(0, crate::drawpipe::Pipe::fixture_view_generation);
        #[cfg(not(target_os = "linux"))]
        let view_gen = 0;
        self.capture((pair, view_gen), input, bytes, newest)
    }

    pub(super) fn capture(
        &mut self,
        change: ((u64, u64), u64),
        input: Input<'_>,
        bytes: &[u8],
        (newest, la): (Option<&Frame>, Option<&[u16]>),
    ) -> Option<Pending> {
        if self.change != Some(change) {
            self.change = Some(change);
            self.left = self.per_change;
        }
        if self.left == 0 || self.next == MAX_BATCHES {
            return None;
        }
        self.left -= 1;
        // The HAL removes per-frame padding in place; all live batch plans are naturally aligned.
        if input.aligned_len() != Some(bytes.len()) {
            eprintln!("oos-ui batch dump: read size differs from the unpadded live geometry");
            return None;
        }
        let raw = newest.map_or(0, |f| f.ch.iter().map(|ch| ch.len() as u64 * 4).sum());
        let la = la.map(|words| (words, input.la_timing())).or_else(|| {
            let record = newest?.la.as_ref()?;
            Some((record.words.as_slice(), (record.t0, record.dt)))
        });
        let charge = bytes.len() as u64 + raw + la.map_or(0, |(words, _)| words.len() as u64 * 2) + META_LIMIT;
        if charge > self.remaining {
            eprintln!("oos-ui batch dump: batch exceeds remaining disk budget");
            return None;
        }
        let path = batch_path(&self.root, self.next);
        self.next += 1;
        self.remaining -= charge;
        write_capture(path, input.encode(&self.build), bytes, newest, la)
            .map_err(|e| eprintln!("oos-ui batch dump: {e}"))
            .ok()
    }
}

#[cfg(target_os = "linux")]
fn valid_build(build: &str) -> bool {
    let Some((date, kernel)) = build.split_once(" kernel ") else { return false };
    date.len() == 15
        && date.as_bytes()[8] == b'-'
        && date.bytes().enumerate().all(|(i, c)| i == 8 || c.is_ascii_digit())
        && (1..=64).contains(&kernel.len())
        && kernel.split(['.', '-', '+']).next().is_some_and(|v| v.parse::<u32>().is_ok())
        && kernel.bytes().all(|c| c.is_ascii_digit() || b".-+".contains(&c))
}

fn create_private(path: &Path) -> io::Result<()> {
    DirBuilder::new().mode(0o700).create(path)
}

fn batch_path(root: &Path, slot: usize) -> PathBuf {
    root.join(format!("batch-{slot:03}"))
}

pub(in crate::board::dho::hw) struct Pending {
    path: PathBuf,
    meta: String,
    dma_io: Duration,
}

fn write_capture(
    path: PathBuf,
    mut meta: String,
    bytes: &[u8],
    newest: Option<&Frame>,
    la: Option<(&[u16], (f64, f64))>,
) -> io::Result<Pending> {
    use std::fmt::Write as _;
    let started = Instant::now();
    create_private(&path)?;
    fs::write(path.join("dma.bin"), bytes)?;
    if let Some(frame) = newest {
        let [a, b, c, d] = frame.ch.each_ref().map(Vec::len);
        let _ = writeln!(
            meta,
            "newest={} {} {} {} {} {} {a} {b} {c} {d}",
            frame.dt, frame.pre, frame.acq.0, frame.acq.1, frame.envelope, frame.pre_exact
        );
        let fixed_ti = frame.fixed_ti.map_or_else(|| "none".into(), |ti| ti.to_string());
        let _ = writeln!(meta, "newest_flags={} {} {} {fixed_ti}", frame.roll, frame.scan, frame.partial);
        write_words(&path.join("newest.bin"), frame.ch.iter().flatten().map(|sample| sample.to_le_bytes()))?;
    }
    if let Some((words, (t0, dt))) = la {
        let _ = writeln!(meta, "la={} {t0} {dt}", words.len());
        write_words(&path.join("la.bin"), words.iter().map(|word| word.to_le_bytes()))?;
    }
    Ok(Pending { path, meta, dma_io: started.elapsed() })
}

fn write_words<const N: usize>(path: &Path, words: impl Iterator<Item = [u8; N]>) -> io::Result<()> {
    let mut out = BufWriter::new(File::create(path)?);
    for word in words {
        out.write_all(&word)?;
    }
    out.flush()
}

impl Pending {
    pub(in crate::board::dho::hw) fn trace(self) -> Box<RouteTrace> {
        RouteTrace::new(move |report| self.finish(Some(report), "batch"))
    }

    pub(in crate::board::dho::hw) fn finish_records(self) {
        self.finish(None, "records");
    }

    fn finish(mut self, report: Option<RouteReport>, dispatch: &str) {
        use std::fmt::Write as _;
        let _ = writeln!(self.meta, "dispatch={dispatch}");
        if let Some(report) = report {
            let range = report.range();
            let _ = writeln!(self.meta, "submitted={} {}\n{}", range.start, range.end, report.route());
            report.append_view(&mut self.meta);
        }
        let path = self.path.join("fixture.txt.part");
        let result = self.write_sidecar(&path).and_then(|()| fs::rename(path, self.path.join("fixture.txt")));
        if let Err(error) = result {
            eprintln!("oos-ui batch dump: {error}");
        }
    }

    fn write_sidecar(&self, path: &Path) -> io::Result<()> {
        // Reserve space for the reporting field itself.
        if self.meta.len() as u64 + 64 > META_LIMIT {
            return Err(io::Error::other("sidecar exceeds disk budget"));
        }
        let started = Instant::now();
        let mut file = File::create(path)?;
        file.write_all(self.meta.as_bytes())?;
        writeln!(file, "dump_io_ns={}", (self.dma_io + started.elapsed()).as_nanos())
    }
}

pub(in crate::board::dho::hw) struct Captured<T> {
    buffer: T,
    trace: Option<Box<RouteTrace>>,
}

impl<T: crate::drawpipe::RawBatch + 'static> Captured<T> {
    pub(in crate::board::dho::hw) fn wrap(
        buffer: T,
        trace: Option<Box<RouteTrace>>,
    ) -> Box<dyn crate::drawpipe::RawBatch> {
        match trace {
            Some(trace) => Box::new(Self { buffer, trace: Some(trace) }),
            None => Box::new(buffer),
        }
    }
}

impl<T: crate::drawpipe::RawBatch + 'static> crate::drawpipe::RawBatch for Captured<T> {
    fn bytes(&self) -> &[u8] {
        self.buffer.bytes()
    }

    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any + Send> {
        Box::new(self.buffer).into_any()
    }

    fn take_route_trace(&mut self) -> Option<Box<RouteTrace>> {
        self.trace.take()
    }
}
