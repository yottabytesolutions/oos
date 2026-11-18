// SPDX-License-Identifier: GPL-2.0-or-later
//! Owned application messages shared by synchronous intake and transport adapters.
pub(crate) mod hislip;
mod latency;
mod output;
pub(crate) mod rpc;
pub use crate::system::intake::{Id, Port, Wake};
pub use output::Output;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

pub const MAX_PROGRAM: usize = 4 << 20;
pub const MAX_CLIENTS: usize = 8;
pub const RESPONSE_BYTES: usize = 64 << 20;
pub const IMPORT_BYTES: usize = 128 << 20;
pub const UPLOAD_BYTES: usize = 70 << 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    Local,
    Http,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WaveFormat {
    Csv,
    Binary,
}

/// An admitted body owns its allocation; transport readers enforce the same limit before allocating.
pub struct Payload(Vec<u8>);
impl Payload {
    pub fn new(bytes: Vec<u8>) -> Result<Self, Vec<u8>> {
        if bytes.capacity() > IMPORT_BYTES {
            Err(bytes)
        } else {
            Ok(Self(bytes))
        }
    }
    pub fn bytes(&self) -> &[u8] {
        &self.0
    }
}

pub enum RemoteRequest {
    Program(String),
    Vxi11 { record: Vec<u8>, channel: rpc::Channel, closed: Arc<AtomicBool> },
    Hislip { message: Option<hislip::Message>, closed: Arc<AtomicBool>, cleared: Arc<AtomicBool> },
    UiState,
    ImportWaveform { slot: usize, name: String, payload: Payload },
    ExportWaveform { format: WaveFormat },
    ExportLogicCsv,
    Stamp,
    UpdateStatus,
    Cmd { lines: Vec<String>, origin: Origin },
}

pub struct Envelope {
    id: Id,
    request: RemoteRequest,
    cancelled: Arc<AtomicBool>,
    read_closed: Arc<AtomicBool>,
}
impl Envelope {
    pub fn message(connection: u64, request: u64, message: RemoteRequest) -> Self {
        Self::new(Id::new(connection, request), message)
    }
    #[cfg(test)]
    pub fn program(connection: u64, request: u64, text: String) -> Self {
        Self::new(Id::new(connection, request), RemoteRequest::Program(text))
    }
    pub fn new(id: Id, request: RemoteRequest) -> Self {
        Self { id, request, cancelled: Arc::new(AtomicBool::new(false)), read_closed: Arc::new(AtomicBool::new(false)) }
    }
    pub fn credit(&self, ports: Arc<Ports>) -> Credit {
        Credit { ports, id: self.id, cancelled: self.cancelled.clone() }
    }
    pub fn id(&self) -> Id {
        self.id
    }
    pub fn request(&self) -> &RemoteRequest {
        &self.request
    }
    pub fn cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }
    pub fn read_closed(&self) -> Arc<AtomicBool> {
        self.read_closed.clone()
    }
}

/// The IO task holds this until output completion. Cancellation wakes frame intake.
pub struct Credit {
    ports: Arc<Ports>,
    id: Id,
    cancelled: Arc<AtomicBool>,
}
impl Drop for Credit {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
        self.ports.requests().cancel(self.id);
        self.ports.replies().cancel(self.id);
        self.ports.notify_frame();
    }
}

/// Reservations cover assembled, parked, queued and IO-held output, without charging moves twice.
pub struct Budget {
    used: Mutex<usize>,
    limit: usize,
    wake: Arc<Wake>,
}
impl Budget {
    #[cfg(test)]
    pub fn new(wake: Arc<Wake>) -> Arc<Self> {
        Self::bounded(wake, RESPONSE_BYTES)
    }
    fn bounded(wake: Arc<Wake>, limit: usize) -> Arc<Self> {
        Arc::new(Self { used: Mutex::new(0), limit, wake })
    }
    pub fn reserve(self: &Arc<Self>, bytes: usize) -> Option<Reservation> {
        let mut used = oos_hal::sync::lock(&self.used);
        if bytes > self.limit.saturating_sub(*used) {
            return None;
        }
        *used += bytes;
        Some(Reservation { budget: self.clone(), bytes })
    }
    pub fn reserve_available(self: &Arc<Self>) -> Option<Reservation> {
        let available = self.limit.saturating_sub(self.used());
        (available != 0).then(|| self.reserve(available)).flatten()
    }
    pub fn used(&self) -> usize {
        *oos_hal::sync::lock(&self.used)
    }
}
pub struct Reservation {
    budget: Arc<Budget>,
    bytes: usize,
}
impl Reservation {
    pub fn capacity(&self) -> usize {
        self.bytes
    }
    pub fn retain(&mut self, bytes: usize) {
        assert!(bytes <= self.bytes, "response allocation exceeds its reservation");
        *oos_hal::sync::lock(&self.budget.used) -= self.bytes - bytes;
        self.bytes = bytes;
        self.budget.wake.notify();
    }
}
impl Drop for Reservation {
    fn drop(&mut self) {
        *oos_hal::sync::lock(&self.budget.used) -= self.bytes;
        self.budget.wake.notify();
    }
}

pub struct Reply {
    id: Id,
    output: Output,
    _reservation: Reservation,
    close: bool,
}
impl Reply {
    pub fn new(id: Id, bytes: Vec<u8>, reservation: Reservation) -> Self {
        let mut output = Output::default();
        if !bytes.is_empty() || bytes.capacity() != 0 {
            assert!(output.push(bytes).is_ok(), "response exceeds reserved capacity");
        }
        Self::from_output(id, output, reservation)
    }
    pub fn from_output(id: Id, output: Output, mut reservation: Reservation) -> Self {
        reservation.retain(output.capacity());
        Self { id, output, _reservation: reservation, close: false }
    }
    pub fn id(&self) -> Id {
        self.id
    }
    pub(crate) fn set_close(&mut self, close: bool) {
        self.close = close;
    }
    pub(crate) fn closes(&self) -> bool {
        self.close
    }
    pub fn output(&self) -> &Output {
        &self.output
    }
    pub fn capacity(&self) -> usize {
        self.output.capacity()
    }
}

pub struct Ports {
    requests: Port<Envelope>,
    replies: Port<Reply>,
    budget: Arc<Budget>,
    frame_wake: Arc<Wake>,
    io_wake: Arc<Wake>,
    enqueue: latency::Latency,
}
impl Ports {
    pub fn new(frame_wake: Arc<Wake>) -> std::io::Result<Arc<Self>> {
        Self::bounded(frame_wake, MAX_CLIENTS, RESPONSE_BYTES)
    }
    pub(crate) fn bounded(frame_wake: Arc<Wake>, requests: usize, reply_bytes: usize) -> std::io::Result<Arc<Self>> {
        let io_wake = Arc::new(Wake::new()?);
        Ok(Arc::new(Self {
            requests: Port::new(requests, requests * MAX_PROGRAM, frame_wake.clone()),
            replies: Port::new(1, reply_bytes, io_wake.clone()),
            budget: Budget::bounded(frame_wake.clone(), reply_bytes),
            frame_wake,
            io_wake,
            enqueue: Default::default(),
        }))
    }
    pub(crate) fn enqueue(&self) -> &latency::Latency {
        &self.enqueue
    }
    pub fn requests(&self) -> &Port<Envelope> {
        &self.requests
    }
    pub fn replies(&self) -> &Port<Reply> {
        &self.replies
    }
    /// Retry an owned reply without releasing its reservation or repeating interpretation.
    pub fn flush(&self, parked: &mut Option<Reply>) -> bool {
        if let Some(reply) = parked.take() {
            let capacity = reply.capacity();
            *parked = self.replies.submit(reply.id(), reply, capacity).err();
        }
        parked.is_none()
    }
    pub fn budget(&self) -> &Arc<Budget> {
        &self.budget
    }
    pub fn io_wake(&self) -> Arc<Wake> {
        self.io_wake.clone()
    }
    pub fn notify_frame(&self) {
        self.frame_wake.notify();
    }
    pub fn close(&self) {
        self.requests.clear(true);
        self.replies.clear(true);
    }
}

/// Signed uploads have a separate single-large-body permit. Only this job identity crosses intake.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UploadJob(u64);
impl UploadJob {
    pub fn new(id: u64) -> Self {
        Self(id)
    }
    pub fn id(self) -> u64 {
        self.0
    }
}
pub struct UploadAdmission(AtomicBool);
impl Default for UploadAdmission {
    fn default() -> Self {
        Self(AtomicBool::new(false))
    }
}
impl UploadAdmission {
    pub fn admit(&self, bytes: usize) -> Option<UploadPermit<'_>> {
        (bytes <= UPLOAD_BYTES && self.0.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire).is_ok())
            .then(|| UploadPermit(self))
    }
}
pub struct UploadPermit<'a>(&'a UploadAdmission);
impl Drop for UploadPermit<'_> {
    fn drop(&mut self) {
        self.0 .0.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests;
