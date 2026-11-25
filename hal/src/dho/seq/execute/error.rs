// SPDX-License-Identifier: GPL-2.0-or-later

use std::fmt;

/// A failed sequencer step, with C2H transfer errors distinguished from other failures.
#[derive(Debug, PartialEq, Eq)]
pub enum ExecuteError {
    DmaRead(String),
    DmaTimeout(String),
    Other(String),
}

impl ExecuteError {
    /// The diagnostic produced by the failed step.
    pub fn message(&self) -> &str {
        match self {
            Self::DmaRead(text) | Self::DmaTimeout(text) | Self::Other(text) => text,
        }
    }
}

impl fmt::Display for ExecuteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message())
    }
}

impl std::error::Error for ExecuteError {}

impl From<ExecuteError> for String {
    fn from(error: ExecuteError) -> Self {
        match error {
            ExecuteError::DmaRead(text) | ExecuteError::DmaTimeout(text) | ExecuteError::Other(text) => text,
        }
    }
}

impl From<String> for ExecuteError {
    fn from(text: String) -> Self {
        Self::Other(text)
    }
}

impl From<&str> for ExecuteError {
    fn from(text: &str) -> Self {
        Self::Other(text.to_owned())
    }
}
