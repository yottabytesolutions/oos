// SPDX-License-Identifier: GPL-2.0-or-later
//! Static AArch64 executable validation shared by the link and image checks.
use std::path::Path;

#[cfg(test)]
pub(crate) mod tests;

fn number(data: &[u8], offset: usize, size: usize) -> u64 {
    data[offset..offset + size].iter().rev().fold(0, |n, byte| (n << 8) | u64::from(*byte))
}

pub(crate) fn validate(data: &[u8]) -> Result<(), &'static str> {
    if data.len() < 64 || &data[..7] != b"\x7fELF\x02\x01\x01" {
        return Err("expected little-endian ELF64");
    }
    if number(data, 16, 2) != 2 {
        return Err("expected an ELF executable");
    }
    if number(data, 18, 2) != 183 {
        return Err("expected AArch64 machine");
    }
    let offset = usize::try_from(number(data, 32, 8)).map_err(|_| "ELF header offset is too large")?;
    let size = number(data, 54, 2) as usize;
    let count = number(data, 56, 2) as usize;
    let end = offset.checked_add(size * count).ok_or("truncated ELF program headers")?;
    let headers = data.get(offset..end).ok_or("missing or truncated ELF program headers")?;
    if count == 0 || size < 56 {
        return Err("missing or truncated ELF program headers");
    }
    let entry = number(data, 24, 8);
    let mut executable = false;
    for header in headers.chunks_exact(size) {
        match number(header, 0, 4) {
            2 | 3 => return Err("expected static executable with LOAD and no INTERP/DYNAMIC segments"),
            1 => {
                let (start, address, length, memory) =
                    (number(header, 8, 8), number(header, 16, 8), number(header, 32, 8), number(header, 40, 8));
                if start.checked_add(length).is_none_or(|end| end > data.len() as u64) || memory < length {
                    return Err("truncated ELF LOAD segment");
                }
                executable |=
                    number(header, 4, 4) & 1 != 0 && entry.checked_sub(address).is_some_and(|offset| offset < length);
            }
            _ => {}
        }
    }
    if !executable {
        return Err("ELF entry is outside executable file content");
    }
    Ok(())
}

pub(crate) fn file(path: &Path) -> Result<(), String> {
    let data = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    validate(&data).map_err(|e| format!("{}: {e}", path.display()))
}

pub(crate) fn run(paths: &[&str]) -> bool {
    paths.iter().all(|path| file(Path::new(path)).map_err(|e| eprintln!("{e}")).is_ok())
}
