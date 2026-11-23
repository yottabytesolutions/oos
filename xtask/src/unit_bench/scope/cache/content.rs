// SPDX-License-Identifier: GPL-2.0-or-later

use super::{sha256, Scratch};
use std::{
    collections::HashSet,
    fs::{self, File},
    io::{self, Write},
    os::unix::{ffi::OsStrExt, fs::MetadataExt},
    path::{Path, PathBuf},
    thread,
};

pub(super) fn identity(root: &Path, scratch: &Scratch) -> Result<String, String> {
    let mut files = Vec::new();
    collect_files(root, Path::new(""), &mut files, &mut HashSet::new())?;
    files.sort_unstable_by(|a, b| a.as_os_str().as_bytes().cmp(b.as_os_str().as_bytes()));
    let (input, output) = io::pipe().map_err(|_| "open fixture hash pipe")?;
    thread::scope(|scope| {
        let writer = scope.spawn(|| write_files(root, &files, output));
        let digest = sha256(input, scratch);
        let written = writer.join().map_err(|_| "fixture hash writer panicked")?;
        // A failed hash process can cause a broken pipe. Report the original error first.
        digest.and_then(|digest| written.map(|()| digest))
    })
}

fn collect_files(
    root: &Path,
    path: &Path,
    files: &mut Vec<PathBuf>,
    links: &mut HashSet<(u64, u64)>,
) -> Result<(), String> {
    let full_path = root.join(path);
    let metadata = fs::symlink_metadata(&full_path).map_err(|_| "read fixture metadata")?;
    if metadata.is_dir() {
        for entry in fs::read_dir(full_path).map_err(|_| "read fixture group")? {
            let entry = entry.map_err(|_| "read fixture entry")?;
            collect_files(root, &path.join(entry.file_name()), files, links)?;
        }
    } else if metadata.is_file() && (metadata.nlink() == 1 || links.insert((metadata.dev(), metadata.ino()))) {
        files.push(path.to_owned());
    } else {
        return Err("fixture groups must contain only directories and regular files without archive links".into());
    }
    Ok(())
}

fn write_files(root: &Path, files: &[PathBuf], mut output: impl Write) -> Result<(), String> {
    // Length prefixes distinguish adjacent paths and contents, even when they contain arbitrary bytes.
    let header_error = |_| "write fixture hash header";
    output.write_all(b"oos-bench-fixtures-v1\0").map_err(header_error)?;
    for path in files {
        let name = path.as_os_str().as_bytes();
        let mut file = File::open(root.join(path)).map_err(|_| "open file for fixture hash")?;
        let length = file.metadata().map_err(|_| "read file length for fixture hash")?.len();
        output.write_all(&(name.len() as u64).to_be_bytes()).map_err(header_error)?;
        output.write_all(name).map_err(header_error)?;
        output.write_all(&length.to_be_bytes()).map_err(header_error)?;
        let copied = io::copy(&mut file, &mut output).map_err(|_| "stream file into fixture hash")?;
        if copied != length {
            return Err("file length changed while hashing fixture group".into());
        }
    }
    Ok(())
}
