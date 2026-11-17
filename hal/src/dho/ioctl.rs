// SPDX-License-Identifier: GPL-2.0-or-later

//! Typed buffers and borrowed descriptors at the board's ioctl boundary.

use std::io;
use std::os::fd::{AsFd, AsRawFd};

/// Issues one ioctl and preserves the OS error on failure.
///
/// # Safety
/// `request` must accept a pointer to an initialized `T` on this descriptor, access at most its size,
/// and leave a valid `T` behind. It must not retain the pointer or close the descriptor.
pub(super) unsafe fn ioctl<T>(fd: &impl AsFd, request: libc::c_ulong, data: &mut T) -> io::Result<()> {
    // SAFETY: the caller guarantees the request's buffer contract; the descriptor and buffer stay borrowed.
    if unsafe { libc::ioctl(fd.as_fd().as_raw_fd(), request as _, data as *mut T) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::ioctl;
    use std::fs::File;
    use std::io::Write;
    use std::os::fd::OwnedFd;
    use std::os::unix::net::UnixStream;

    #[test]
    fn returns_the_os_error_for_a_non_device_fd() {
        let file = File::open("/dev/null").unwrap();
        let mut available: libc::c_int = 0;
        // SAFETY: FIONREAD takes an int buffer and neither retains it nor closes the descriptor.
        let error = unsafe { ioctl(&file, libc::FIONREAD as _, &mut available) }.unwrap_err();
        // /dev/null rejects ioctls with ENODEV on Darwin and ENOTTY on Linux.
        assert!(matches!(error.raw_os_error(), Some(libc::ENOTTY | libc::ENODEV)), "{error}");
    }

    #[test]
    fn writes_the_typed_buffer_and_leaves_the_fd_open() {
        let (mut writer, reader) = UnixStream::pair().unwrap();
        writer.write_all(b"gpio").unwrap();
        let fd: OwnedFd = reader.into();
        let mut available: libc::c_int = -1;
        // SAFETY: FIONREAD writes an int without retaining its pointer or closing the stream.
        unsafe { ioctl(&fd, libc::FIONREAD as _, &mut available) }.unwrap();
        assert_eq!(available, 4);
        // SAFETY: the same FIONREAD contract holds while fd is still owned here.
        unsafe { ioctl(&fd, libc::FIONREAD as _, &mut available) }.unwrap();
        assert_eq!(available, 4);
    }
}
