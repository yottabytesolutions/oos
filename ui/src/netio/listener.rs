// SPDX-License-Identifier: GPL-2.0-or-later
//! TCP admission and joined connection lifetime shared by instrument and HTTP listeners.
use std::{future::Future, io};
use tokio::task::JoinSet;

#[derive(Clone, Copy)]
pub(super) enum Capacity {
    Available,
    Full,
}

pub(super) async fn serve<C, A, D, S>(
    maximum: usize,
    stop: tokio_util::sync::CancellationToken,
    mut accept: impl FnMut() -> A,
    mut admit: impl FnMut(C, Capacity) -> D,
) where
    A: Future<Output = io::Result<C>>,
    D: Future<Output = Option<S>>,
    S: Future<Output = ()> + Send + 'static,
{
    let mut clients = JoinSet::new();
    loop {
        tokio::select! {
            _ = stop.cancelled() => break,
            event = accept() => {
                let connection = match event {
                    Ok(connection) => connection,
                    Err(error) => {
                        eprintln!("oos-ui: TCP accept failed: {error}");
                        tokio::select! {
                            _ = stop.cancelled() => break,
                            _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => {},
                        }
                        continue;
                    }
                };
                let session = tokio::select! {
                    _ = stop.cancelled() => break,
                    session = admit(connection, if clients.len() >= maximum { Capacity::Full } else { Capacity::Available }) => session,
                };
                if let Some(session) = session {
                    clients.spawn(session);
                }
            }
            Some(_) = clients.join_next(), if !clients.is_empty() => {}
        }
    }
    clients.abort_all();
    while clients.join_next().await.is_some() {}
}

#[cfg(test)]
mod tests;
