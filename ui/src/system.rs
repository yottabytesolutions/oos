// SPDX-License-Identifier: GPL-2.0-or-later
//! Process lifecycle policy and bounded waits for owned workers.

#[cfg(test)]
pub mod perfguard;
#[cfg(test)]
#[allow(dead_code)] // Shared observer also serves integration soak retained-byte and thread-local probes.
pub(crate) mod test_allocator;

pub mod clocks;
pub mod intake;
pub mod memory;
pub mod serve;
pub mod shutdown;
pub mod wake;

#[cfg(target_os = "linux")]
pub mod device;

pub fn stop_server(
    ui: &mut crate::ui::Ui,
    web: &mut crate::http::Http,
    scpi: &mut crate::scpi::Scpi,
    vxi: &mut crate::vxi11::Server,
) {
    let _ = shutdown::run(
        std::time::Instant::now(),
        ui,
        |deadline| {
            scpi.set_open(None, false);
            vxi.set_open(false, false);
            web.shutdown(deadline);
        },
        |ui, deadline| ui.stop_workers(deadline),
        |_| Ok(()),
        |_| true,
        || {},
    );
    if ui.exit_code() != 0 {
        std::process::exit(ui.exit_code());
    }
}
