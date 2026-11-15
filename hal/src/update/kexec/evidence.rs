// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! One bounded legacy-load attempt on the persistent OOS data partition (docs/boot.md).

use std::io::{self, Read, Write};
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};

#[cfg(target_os = "linux")]
pub(super) const DIR: &str = "/oos/update/kexec-last";
pub(super) const TEXT_LIMIT: usize = 64 << 10;
pub(super) const TREE_LIMIT: usize = 2 << 20;

/// Drain a pipe even after its evidence budget is exhausted, so the child cannot block on output.
pub(super) fn capture(mut input: impl Read) -> io::Result<Vec<u8>> {
    let mut saved = Vec::new();
    let mut buf = [0; 4096];
    loop {
        let n = input.read(&mut buf)?;
        if n == 0 {
            return Ok(saved);
        }
        let discard = (saved.len() + n).saturating_sub(TEXT_LIMIT);
        saved.drain(..discard);
        saved.extend_from_slice(&buf[..n]);
    }
}

pub(super) fn run(command: &mut Command) -> Result<(ExitStatus, Vec<u8>, Vec<u8>), String> {
    let mut child = command.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().map_err(|e| e.to_string())?;
    let stdout = child.stdout.take().expect("stdout was configured as a pipe");
    let stderr = child.stderr.take().expect("stderr was configured as a pipe");
    std::thread::scope(|scope| {
        let out = scope.spawn(|| capture(stdout));
        let err = scope.spawn(|| capture(stderr));
        let status = child.wait().map_err(|e| e.to_string())?;
        let out = out.join().map_err(|_| "stdout capture panicked")?.map_err(|e| e.to_string())?;
        let err = err.join().map_err(|_| "stderr capture panicked")?.map_err(|e| e.to_string())?;
        Ok((status, out, err))
    })
}

pub(super) fn write(dir: &Path, name: &str, bytes: &[u8], limit: usize) -> io::Result<()> {
    let mut file = std::fs::File::create(dir.join(name))?;
    file.write_all(&bytes[..bytes.len().min(limit)])?;
    file.sync_all()?;
    std::fs::File::open(dir)?.sync_all()
}

pub(super) fn begin(
    dir: &Path,
    kernel: usize,
    initrd: usize,
    tree: &[u8],
    from: u64,
    no_checks: bool,
) -> io::Result<()> {
    std::fs::create_dir_all(dir)?;
    // Invalidate previous jump/output evidence before saving the new attempt, including on a load failure.
    for name in ["jump.log", "stdout", "stderr", "merged.dtb", "staged.log", "load.log"] {
        write(dir, name, b"", TEXT_LIMIT)?;
    }
    std::fs::File::open(dir.parent().ok_or_else(|| io::Error::other("evidence has no parent"))?)?.sync_all()?;
    let lengths = format!("kernel_bytes={kernel}\ninitrd_bytes={initrd}\nmerged_dtb_bytes={}\nmem_min=0x{from:x}\npurgatory_no_checks={no_checks}\nstdout_stderr_limit={TEXT_LIMIT} (tail; may be truncated)\n", tree.len());
    write(dir, "staged.log", lengths.as_bytes(), TEXT_LIMIT)?;
    if tree.len() > TREE_LIMIT {
        return Err(io::Error::other("merged DTB exceeds 2 MiB evidence budget; not saved"));
    }
    write(dir, "merged.dtb", tree, TREE_LIMIT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_is_drained_and_bounded() {
        let mut data = vec![b'x'; TEXT_LIMIT * 3];
        data.extend_from_slice(b"last segment");
        let mut input = &data[..];
        let saved = capture(&mut input).unwrap();
        assert_eq!(saved.len(), TEXT_LIMIT);
        assert!(saved.ends_with(b"last segment"));
        assert!(input.is_empty());
        assert!(capture(&b""[..]).unwrap().is_empty());
    }

    #[test]
    fn attempt_overwrites_old_files_and_preserves_complete_tree() {
        let dir = std::env::temp_dir().join(format!("oos-kexec-evidence-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        begin(&dir, 10, 20, b"first tree", 0x40000000, false).unwrap();
        assert!(std::fs::read_to_string(dir.join("staged.log")).unwrap().contains("purgatory_no_checks=false\n"));
        write(&dir, "stderr", &vec![b'x'; TEXT_LIMIT * 2], TEXT_LIMIT).unwrap();
        assert_eq!(std::fs::metadata(dir.join("stderr")).unwrap().len(), TEXT_LIMIT as u64);
        write(&dir, "jump.log", b"old jump", TEXT_LIMIT).unwrap();
        begin(&dir, 30, 40, b"new", 0x80000000, true).unwrap();
        assert_eq!(std::fs::read(dir.join("merged.dtb")).unwrap(), b"new");
        assert_eq!(std::fs::read(dir.join("jump.log")).unwrap(), b"");
        assert_eq!(std::fs::read(dir.join("stderr")).unwrap(), b"");
        let lengths = std::fs::read_to_string(dir.join("staged.log")).unwrap();
        assert!(lengths.contains("kernel_bytes=30\ninitrd_bytes=40\nmerged_dtb_bytes=3\nmem_min=0x80000000"));
        assert!(lengths.contains("purgatory_no_checks=true\n"));
        assert!(begin(&dir, 1, 2, &vec![0; TREE_LIMIT + 1], 0, false).is_err());
        assert!(std::fs::read(dir.join("merged.dtb")).unwrap().is_empty());
        assert!(write(&dir.join("missing"), "stderr", b"x", TEXT_LIMIT).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn captures_both_pipes_with_a_bound_even_on_failure() {
        let (status, out, err) = run(Command::new("sh").args([
            "-c",
            "printf out; i=0; while [ $i -lt 20000 ]; do printf 1234567890 >&2; i=$((i + 1)); done; exit 7",
        ]))
        .unwrap();
        assert_eq!(status.code(), Some(7));
        assert_eq!(out, b"out");
        assert_eq!(err.len(), TEXT_LIMIT);
        assert!(run(&mut Command::new("/path/to/nonexistent-kexec-test")).is_err());
    }
}
