// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! Linux GPIO character device (v1 ABI), lines found by their device-tree names (e.g. `spi_afe_sclk`). Only libc.

use std::ffi::CStr;
use std::fs::{File, OpenOptions};
use std::os::fd::{FromRawFd, OwnedFd};

use super::ioctl::ioctl;

#[derive(Default)]
#[repr(C)]
struct ChipInfo {
    name: [u8; 32],
    label: [u8; 32],
    lines: u32,
}

#[derive(Default)]
#[repr(C)]
struct LineInfo {
    line_offset: u32,
    flags: u32,
    name: [u8; 32],
    consumer: [u8; 32],
}

#[repr(C)]
struct HandleRequest {
    lineoffsets: [u32; 64],
    flags: u32,
    default_values: [u8; 64],
    consumer_label: [u8; 32],
    lines: u32,
    fd: i32,
}

#[repr(C)]
struct HandleData {
    values: [u8; 64],
}

const fn iowr(nr: u32, size: usize) -> libc::c_ulong {
    ((3u32 << 30) | ((size as u32) << 16) | (0xb4 << 8) | nr) as libc::c_ulong
}
const fn ior(nr: u32, size: usize) -> libc::c_ulong {
    ((2u32 << 30) | ((size as u32) << 16) | (0xb4 << 8) | nr) as libc::c_ulong
}
const GET_CHIPINFO: libc::c_ulong = ior(0x01, std::mem::size_of::<ChipInfo>());
const GET_LINEINFO: libc::c_ulong = iowr(0x02, std::mem::size_of::<LineInfo>());
const GET_LINEHANDLE: libc::c_ulong = iowr(0x03, std::mem::size_of::<HandleRequest>());
const GET_VALUES: libc::c_ulong = iowr(0x08, std::mem::size_of::<HandleData>());
const SET_VALUES: libc::c_ulong = iowr(0x09, std::mem::size_of::<HandleData>());
const REQUEST_INPUT: u32 = 1 << 0;
const REQUEST_OUTPUT: u32 = 1 << 1;

/// One requested GPIO line.
pub struct Line {
    fd: OwnedFd,
}

fn cstr(b: &[u8]) -> &str {
    CStr::from_bytes_until_nul(b).ok().and_then(|c| c.to_str().ok()).unwrap_or("")
}

fn err(what: &str, e: std::io::Error) -> std::io::Error {
    std::io::Error::new(e.kind(), format!("{}: {}", what, e))
}

/// Infallible hardware buses log and continue their sequence, with failed reads still treated as low.
#[cfg(any(target_os = "linux", test))]
pub(super) fn best_effort<T: Default>(result: std::io::Result<T>) -> T {
    result.unwrap_or_else(|e| {
        eprintln!("oos gpio: {}", e);
        T::default()
    })
}

/// Finds line `name` on /dev/gpiochip0..9: (chip fd, offset).
fn find(name: &str) -> std::io::Result<(File, u32)> {
    for c in 0..10 {
        let Ok(fd) = OpenOptions::new().read(true).write(true).open(format!("/dev/gpiochip{}", c)) else {
            continue;
        };
        let mut ci = ChipInfo::default();
        // SAFETY: GET_CHIPINFO writes the initialized, repr(C) Linux gpiochip_info buffer.
        if unsafe { ioctl(&fd, GET_CHIPINFO, &mut ci) }.is_err() {
            continue;
        }
        for l in 0..ci.lines {
            let mut li = LineInfo { line_offset: l, ..LineInfo::default() };
            // SAFETY: GET_LINEINFO reads the offset and writes the repr(C) Linux gpioline_info buffer.
            if unsafe { ioctl(&fd, GET_LINEINFO, &mut li) }.is_ok() && cstr(&li.name) == name {
                return Ok((fd, l));
            }
        }
    }
    Err(std::io::Error::new(std::io::ErrorKind::NotFound, format!("GPIO line '{}' not found", name)))
}

impl Line {
    fn request(name: &str, flags: u32, value: u8) -> std::io::Result<Line> {
        let (chip, off) = find(name)?;
        let mut rq = HandleRequest {
            lineoffsets: [0; 64],
            flags,
            default_values: [0; 64],
            consumer_label: [0; 32],
            lines: 1,
            fd: 0,
        };
        rq.lineoffsets[0] = off;
        rq.default_values[0] = value;
        rq.consumer_label[..6].copy_from_slice(b"oos-hw");
        // SAFETY: GET_LINEHANDLE uses the repr(C) Linux gpiohandle_request and returns a new line fd.
        unsafe { ioctl(&chip, GET_LINEHANDLE, &mut rq) }.map_err(|e| err(name, e))?;
        // SAFETY: a successful GET_LINEHANDLE transfers a fresh, valid descriptor to this sole owner.
        Ok(Line { fd: unsafe { OwnedFd::from_raw_fd(rq.fd) } })
    }
    /// Output line with initial value `value`.
    pub fn output(name: &str, value: bool) -> std::io::Result<Line> {
        Line::request(name, REQUEST_OUTPUT, value as u8)
    }
    pub fn input(name: &str) -> std::io::Result<Line> {
        Line::request(name, REQUEST_INPUT, 0)
    }
    pub fn set(&self, v: bool) -> std::io::Result<()> {
        let mut d = HandleData { values: [0; 64] };
        d.values[0] = v as u8;
        // SAFETY: SET_VALUES reads the initialized repr(C) Linux gpiohandle_data buffer.
        unsafe { ioctl(&self.fd, SET_VALUES, &mut d) }
    }
    pub fn get(&self) -> std::io::Result<bool> {
        let mut d = HandleData { values: [0; 64] };
        // SAFETY: GET_VALUES writes the repr(C) Linux gpiohandle_data buffer of this line handle.
        unsafe { ioctl(&self.fd, GET_VALUES, &mut d) }?;
        Ok(d.values[0] != 0)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn line_reads_and_writes_return_ioctl_errors() {
        let line = super::Line { fd: std::fs::File::open("/dev/null").unwrap().into() };
        assert!(line.set(true).is_err());
        assert!(line.set(false).is_err());
        assert!(line.get().is_err());
    }

    #[test]
    fn infallible_buses_keep_successful_values_and_default_on_failure() {
        assert!(super::best_effort(Ok(true)));
        assert!(!super::best_effort(Ok(false)));
        assert!(!super::best_effort::<bool>(Err(std::io::Error::from_raw_os_error(libc::ENOTTY))));
        super::best_effort(Ok(()));
        super::best_effort::<()>(Err(std::io::Error::from_raw_os_error(libc::ENOTTY)));
    }

    #[test]
    fn ioctl_numbers_match_linux_headers() {
        // linux/gpio.h v1: GPIO_GET_CHIPINFO_IOCTL 0x8044b401, GPIO_GET_LINEINFO_IOCTL 0xc048b402,
        // GPIO_GET_LINEHANDLE_IOCTL 0xc16cb403, GPIOHANDLE_GET/SET_LINE_VALUES_IOCTL 0xc040b408 / 0xc040b409
        assert_eq!(super::GET_CHIPINFO as u32, 0x8044_b401);
        assert_eq!(super::GET_LINEINFO as u32, 0xc048_b402);
        assert_eq!(super::GET_LINEHANDLE as u32, 0xc16c_b403);
        assert_eq!(super::GET_VALUES as u32, 0xc040_b408);
        assert_eq!(super::SET_VALUES as u32, 0xc040_b409);
    }
}
