// SPDX-License-Identifier: GPL-2.0-or-later
//! Upload bytes stay on their worker; intake receives only the header and a job handle.
use std::{io, sync::mpsc, thread::JoinHandle};
type Work = Box<dyn FnOnce(&[u8]) -> Result<(), String> + Send>;
pub(crate) struct Upload {
    header: Vec<u8>,
    len: usize,
    work: mpsc::SyncSender<Work>,
    thread: JoinHandle<Result<(), String>>,
}
impl Upload {
    pub(crate) fn new(body: Vec<u8>) -> io::Result<Self> {
        let header = body[..body.len().min(oos_hal::update::pkg::MAX_HEADER)].to_vec();
        let len = body.len();
        let (work, receive) = mpsc::sync_channel::<Work>(1);
        let thread = std::thread::Builder::new().name("update-check".into()).spawn(move || {
            let work = receive.recv().map_err(|_| "upload cancelled".to_string())?;
            work(&body)
        })?;
        Ok(Self { header, len, work, thread })
    }
    pub(crate) fn header(&self) -> &[u8] {
        &self.header
    }
    pub(crate) fn len(&self) -> usize {
        self.len
    }
    pub(crate) fn run(
        self,
        work: impl FnOnce(&[u8]) -> Result<(), String> + Send + 'static,
    ) -> Result<JoinHandle<Result<(), String>>, String> {
        self.work.try_send(Box::new(work)).map_err(|_| "upload worker stopped".to_string())?;
        Ok(self.thread)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_the_bounded_header_and_job_cross_intake() {
        let body = vec![42; 5 << 20];
        let allocation = body.as_ptr() as usize;
        let caller = std::thread::current().id();
        let job = Upload::new(body).unwrap();
        assert_eq!(job.header(), &[42; oos_hal::update::pkg::MAX_HEADER]);
        assert_eq!(job.len(), 5 << 20);
        job.run(move |bytes| {
            assert_ne!(std::thread::current().id(), caller);
            assert_eq!(bytes.as_ptr() as usize, allocation);
            assert_eq!(bytes.len(), 5 << 20);
            Ok(())
        })
        .unwrap()
        .join()
        .unwrap()
        .unwrap();
    }
}
