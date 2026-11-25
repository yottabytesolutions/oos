// SPDX-License-Identifier: GPL-2.0-or-later

use super::{
    super::{readout::unpad_stride, Step},
    ExecuteError,
};
use crate::dho::bus::{Bus, DmaBuf};
use std::{fmt::Write as _, io};

/// Prefix of an XDMA timeout diagnostic, distinct from trigger and status polls.
pub const DMA_TIMEOUT: &str = "c2h DMA timeout";

pub(super) fn read_play(
    before: &[Step],
    (frames, frame, stride): (usize, usize, usize),
    bar: &mut impl Bus,
    c2h: &mut impl io::Read,
    dma: &mut DmaBuf,
    log: &mut impl FnMut(String),
) -> Result<(), ExecuteError> {
    let planned = frames * stride;
    #[cfg(feature = "private-picture-counts")]
    let bytes = dma.observer_read_bytes(planned);
    #[cfg(not(feature = "private-picture-counts"))]
    let bytes = planned;
    let mut received = 0;
    let buf = dma
        .read_with_progress(
            c2h,
            bytes,
            |got, k| log(format!("c2h: short read of {k} bytes (at {got} of {bytes})")),
            &mut received,
        )
        .map_err(|error| {
            if error.kind() == io::ErrorKind::TimedOut {
                let writes = before.iter().rposition(|step| matches!(step, Step::Wr(_))).map_or(&[][..], |last| {
                    let first = before[..last].iter().rposition(|step| !matches!(step, Step::Wr(_)));
                    &before[first.map_or(0, |index| index + 1)..=last]
                });
                let writes = writes.iter().filter_map(|step| if let Step::Wr(w) = step { Some(*w) } else { None });
                ExecuteError::DmaTimeout(timeout(bar, (bytes, received), writes))
            } else {
                ExecuteError::DmaRead(format!("c2h read of {bytes} bytes: {error} (0x4034 = {:#x})", bar.rd(0x4034)))
            }
        })?;
    #[cfg(feature = "private-picture-counts")]
    if bytes != planned {
        dma.truncate(0);
        return Err(ExecuteError::DmaRead(format!(
            "c2h read of {bytes} bytes: observer's oversized read unexpectedly completed"
        )));
    }
    let n = unpad_stride(buf, frame, stride);
    dma.truncate(n);
    Ok(())
}

/// Format a C2H timeout using the writes that preceded it and one status snapshot.
pub fn timeout(
    bar: &mut impl Bus,
    (asked, received): (usize, usize),
    writes: impl IntoIterator<Item = (u32, u32)>,
) -> String {
    let mut text = format!("{DMA_TIMEOUT}: asked={asked} received={received}");
    for reg in [0x4034, 0x4080, 0x4010, 0x4014] {
        match bar.try_rd(reg) {
            Ok(value) => write!(&mut text, " {reg:#06x}={value:#010x}"),
            Err(error) => write!(&mut text, " {reg:#06x}=unavailable({error})"),
        }
        .expect("writing to a String cannot fail");
    }
    text.push_str(" writes=[");
    for (index, (reg, value)) in writes.into_iter().enumerate() {
        let separator = if index == 0 { "" } else { " " };
        write!(&mut text, "{separator}{reg:#06x}={value:#010x}").expect("writing to a String cannot fail");
    }
    text.push(']');
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dho::bus::Recorder;

    #[test]
    fn timeout_reports_completed_reads_and_only_the_current_play() {
        let mut stream = io::Cursor::new([0; 8]);
        let mut calls = 0;
        let mut read = ReadFn(|buf: &mut [u8]| {
            calls += 1;
            if calls == 1 {
                io::Read::read(&mut stream, buf)
            } else {
                Err(io::ErrorKind::TimedOut.into())
            }
        });
        let steps = [
            Step::Wr((0x1234, 7)),
            Step::Note("read-out".into()),
            Step::Wr((0x11f0, 8)),
            Step::Peek(&[]),
            Step::Read { frames: 1, frame: 16 },
        ];
        let mut bar = Recorder::default();
        bar.regs.extend([(0x4034, 0x1187), (0x4080, 0x4000_0010), (0x4010, 3), (0x4014, 1)]);
        let mut dma = DmaBuf::new();
        let detail = crate::dho::seq::execute_bus(&steps, &mut bar, &mut read, &mut dma, |_| {}).unwrap_err();
        assert_eq!(
            detail,
            "c2h DMA timeout: asked=16 received=8 0x4034=0x00001187 0x4080=0x40000010 0x4010=0x00000003 0x4014=0x00000001 writes=[0x11f0=0x00000008]"
        );
        assert!(dma.data().is_empty());
    }

    struct ReadFn<F>(F);

    impl<F: FnMut(&mut [u8]) -> io::Result<usize>> io::Read for ReadFn<F> {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            (self.0)(buf)
        }
    }

    #[cfg(feature = "private-picture-counts")]
    #[test]
    fn observer_asks_for_an_extra_word_once_and_leaves_no_batch_data() {
        let steps = [Step::Read { frames: 1, frame: 16 }];
        let mut dma = DmaBuf::new();
        let mut bar = Recorder::default();
        let read = |requested| {
            ReadFn(move |buf: &mut [u8]| {
                assert_eq!(buf.len(), requested);
                if requested > 16 {
                    Err(io::ErrorKind::TimedOut.into())
                } else {
                    buf.fill(1);
                    Ok(buf.len())
                }
            })
        };
        dma.stall_next_play();
        let error = crate::dho::seq::execute_bus(&steps, &mut bar, &mut read(24), &mut dma, |_| {}).unwrap_err();
        assert!(error.starts_with("c2h DMA timeout: asked=24 received=0"));
        assert!(dma.data().is_empty());
        crate::dho::seq::execute_bus(&steps, &mut bar, &mut read(16), &mut dma, |_| {}).unwrap();
        assert_eq!(dma.data(), &[1; 16]);
    }
}
