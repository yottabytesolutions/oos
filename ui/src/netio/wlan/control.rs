// SPDX-License-Identifier: GPL-2.0-or-later
use oos_hal::{update::hash::hex, wlan::*};
use std::{io, os::unix::net::UnixDatagram, path::PathBuf, time::Duration};

/// One open control connection.
pub(super) struct Ctrl {
    sock: tokio::net::UnixDatagram,
    own: Option<PathBuf>,
}

impl Drop for Ctrl {
    fn drop(&mut self) {
        if let Some(own) = self.own.take() {
            tokio::task::spawn_blocking(move || std::fs::remove_file(own));
        }
    }
}

impl Ctrl {
    pub(super) async fn open(path: PathBuf) -> io::Result<Self> {
        tokio::task::spawn_blocking(move || {
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(io::Error::other)?
                .as_nanos();
            let own = PathBuf::from("/tmp").join(format!("oos-wlan-{}-{stamp:x}.sock", std::process::id()));
            let sock = UnixDatagram::bind(&own)?;
            let result = sock.connect(path).and_then(|()| sock.set_nonblocking(true));
            if let Err(e) = result {
                let _ = std::fs::remove_file(own);
                return Err(e);
            }
            match tokio::net::UnixDatagram::from_std(sock) {
                Ok(sock) => Ok(Self { sock, own: Some(own) }),
                Err(e) => {
                    let _ = std::fs::remove_file(own);
                    Err(e)
                }
            }
        })
        .await
        .map_err(io::Error::other)?
    }

    pub(super) async fn close(mut self) {
        if let Some(own) = self.own.take() {
            let _ = tokio::task::spawn_blocking(move || std::fs::remove_file(own)).await;
        }
    }

    /// One command and its answer, with the original per-transfer five-second limits.
    pub(super) async fn ask(&self, cmd: &str) -> io::Result<String> {
        timed(self.sock.send(cmd.as_bytes()), tokio::time::sleep(Duration::from_secs(5))).await?;
        let mut b = vec![0u8; 16 * 1024];
        let n = timed(self.sock.recv(&mut b), tokio::time::sleep(Duration::from_secs(5))).await?;
        Ok(String::from_utf8_lossy(&b[..n]).into_owned())
    }

    /// A command that answers OK.
    async fn ok(&self, cmd: &str) -> Result<(), String> {
        let word = cmd.split(' ').next().unwrap_or(cmd);
        match self.ask(cmd).await {
            Ok(a) if a.trim() == "OK" => Ok(()),
            Ok(a) => Err(format!("{}: {}", word, a.trim())),
            Err(e) => Err(format!("{}: {}", word, e)),
        }
    }

    pub(super) async fn status(&self) -> io::Result<Status> {
        self.ask("STATUS").await.map(|t| parse_status(&t))
    }

    pub(super) async fn signal(&self) -> Option<i32> {
        parse_signal(&self.ask("SIGNAL_POLL").await.ok()?)
    }

    /// Starts a scan (a running one is as good as a new one).
    pub(super) async fn scan(&self) -> Result<(), String> {
        match self.ask("SCAN").await {
            Ok(a) if matches!(a.trim(), "OK" | "FAIL-BUSY") => Ok(()),
            Ok(a) => Err(format!("SCAN: {}", a.trim())),
            Err(e) => Err(format!("SCAN: {}", e)),
        }
    }

    pub(super) async fn scan_results(&self) -> io::Result<Vec<Net>> {
        self.ask("SCAN_RESULTS").await.map(|t| parse_scan(&t))
    }

    pub(super) async fn networks(&self) -> io::Result<Vec<Saved>> {
        self.ask("LIST_NETWORKS").await.map(|t| parse_networks(&t))
    }

    /// A new network, not saved yet and not selected; `key` None = open. Also found hidden.
    pub(super) async fn add(&self, ssid: &[u8], key: Option<&[u8; 32]>) -> Result<u32, String> {
        let a = self.ask("ADD_NETWORK").await.map_err(|e| e.to_string())?;
        let id: u32 = a.trim().parse().map_err(|_| format!("ADD_NETWORK: {}", a.trim()))?;
        let set = async {
            self.ok(&format!("SET_NETWORK {} ssid {}", id, hex(ssid))).await?;
            self.ok(&format!("SET_NETWORK {} scan_ssid 1", id)).await?;
            match key {
                Some(k) => self.ok(&format!("SET_NETWORK {} psk {}", id, hex(k))).await,
                None => self.ok(&format!("SET_NETWORK {} key_mgmt NONE", id)).await,
            }
        };
        if let Err(e) = set.await {
            let _ = self.remove(id).await;
            return Err(e);
        }
        Ok(id)
    }

    /// Joins this network now (the others are left alone until `enable_all`).
    pub(super) async fn select(&self, id: u32) -> Result<(), String> {
        self.ok(&format!("SELECT_NETWORK {}", id)).await
    }

    /// After a selection: every saved network may be joined again, this one stays if it is connected.
    pub(super) async fn enable_all(&self) -> Result<(), String> {
        self.ok("ENABLE_NETWORK all").await
    }

    pub(super) async fn remove(&self, id: u32) -> Result<(), String> {
        self.ok(&format!("REMOVE_NETWORK {}", id)).await
    }

    /// Writes the networks to wpa_supplicant's file.
    pub(super) async fn save(&self) -> Result<(), String> {
        self.ok("SAVE_CONFIG").await
    }
}

async fn timed<T>(
    operation: impl std::future::Future<Output = io::Result<T>>,
    timeout: impl std::future::Future<Output = ()>,
) -> io::Result<T> {
    tokio::select! {
        result = operation => result,
        () = timeout => Err(io::Error::new(io::ErrorKind::TimedOut, "WLAN control timed out")),
    }
}

#[cfg(test)]
mod tests;
