// SPDX-License-Identifier: GPL-2.0-or-later
//! Host frame pacing and wakeable remote intake.
use crate::*;

/// No display and no hardware: the demo source behind SCPI, the web server, VXI-11 and HiSLIP, for
/// tools/device-test.py on a host. Every listener is private (loopback, a free port, `net6::Port::Host`); the
/// harness learns the ports from the readiness record. `OOS_DEMO_LOOP=<mask>` puts the generator on those channels,
/// as a cable from the output would.
pub fn run(mut ui: Ui, sw: &sysconf::Switches) {
    let mut web = http::Http::open(net6::Port::Host, sw);
    let mut scpi = scpi::Scpi::open_with_wake(net6::Port::Host, false, web.wake_handle());
    let mut vxi = net6::host_or_exit(vxi11::Server::host(net6::host_or_exit(scpi.io_runtime()), web.wake_handle()));
    let mut hislip = hislip::Server::open(net6::Port::Host);
    web.offer(http::Offer { scpi_port: scpi.port(), vxi11: vxi.service_port().is_some(), hislip_port: hislip.port() });
    net6::host_or_exit(net6::host_ready(web.port(), scpi.port(), vxi.ports(), hislip.port()));
    let mut cv = Canvas::new(ui.scr.w, ui.scr.h);
    let mut last = std::time::Instant::now() - std::time::Duration::from_millis(10);
    while !ui.quit {
        if last.elapsed() >= std::time::Duration::from_millis(10) {
            ui.step(0.04);
            ui.render(&mut cv);
            last = std::time::Instant::now();
        }
        scpi.poll(&mut ui, &cv);
        vxi.poll_async(&mut |program| scpi.poll_program(&mut ui, &cv, program));
        hislip.poll_async(&mut |program| scpi.poll_program(&mut ui, &cv, program));
        for c in web.poll(&mut ui, &cv, &mut scpi) {
            cmd::run(&mut ui, &c);
        }
        let wait = std::time::Duration::from_millis(10).saturating_sub(last.elapsed());
        web.wake().wait(wait.as_nanos().div_ceil(1_000_000) as i32);
    }
    system::stop_server(&mut ui, &mut web, &mut scpi, &mut vxi);
}
