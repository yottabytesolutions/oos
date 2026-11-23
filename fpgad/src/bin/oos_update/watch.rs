// SPDX-License-Identifier: GPL-2.0-or-later

use oos_hal::update::State;

#[cfg(target_os = "linux")]
mod system;
#[cfg(target_os = "linux")]
pub(crate) use system::run;

enum Outcome {
    Good,
    Bad(String),
    UiOff,
}

trait Ports {
    fn verdict(&mut self) -> Outcome;
    fn confirm(&mut self, state: &State, sha: [u8; 32]);
    fn arm(&mut self) -> bool;
    fn rollback(&mut self, state: &State, sha: [u8; 32], why: &str);
    fn unhealthy(&mut self, why: &str);
    fn publish(&mut self, outcome: &str);
}

fn watch(ports: &mut impl Ports, state: &State, kexec: bool, running: Option<&str>) {
    if kexec {
        // A kexec session cannot confirm or roll back a trial. A persist arm must finish before publication.
        let armed = state.persist && state.trial.is_none() && ports.arm();
        ports.publish(if armed { "persist" } else { "kexec" });
        return;
    }
    let trial_sha = state
        .current
        .as_ref()
        .filter(|entry| state.trial.is_some() && running == Some(entry.build.as_str()))
        .map(|entry| entry.sha256);
    let marker = match ports.verdict() {
        Outcome::Good => {
            if let Some(sha) = trial_sha {
                ports.confirm(state, sha);
            }
            if !state.persist {
                "oneshot"
            } else if ports.arm() {
                "persist"
            } else {
                "stock"
            }
        }
        Outcome::Bad(why) => {
            if let Some(sha) = trial_sha.filter(|_| state.previous.is_some()) {
                ports.rollback(state, sha, &why);
            } else {
                ports.unhealthy(&why);
            }
            "stock"
        }
        Outcome::UiOff => "ui-off",
    };
    ports.publish(marker);
}

fn publish_to(path: &std::path::Path, outcome: &str) -> std::io::Result<()> {
    let temporary = path.with_extension("new");
    std::fs::write(&temporary, outcome)?;
    std::fs::rename(temporary, path)
}

#[cfg(test)]
mod tests;
