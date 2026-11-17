// SPDX-License-Identifier: GPL-2.0-or-later
//! Locking without `unwrap` (the gate denies new `unwrap` outside tests, docs/development.md).

use std::sync::{Mutex, MutexGuard, PoisonError};

/// Locks `m`. A lock poisoned by a panicking holder still gives its data: the scope's binaries abort on panic
/// (`panic = "abort"` in ui and fpgad), so poisoning happens only in tests, where the panic is the report already.
pub fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_poisoned_lock_still_gives_its_data() {
        let m = std::sync::Arc::new(Mutex::new(5));
        let m2 = m.clone();
        let poisoned = std::thread::spawn(move || {
            let _g = lock(&m2);
            panic!("poison it");
        })
        .join();
        assert!(poisoned.is_err() && m.is_poisoned());
        *lock(&m) += 1;
        assert_eq!(*lock(&m), 6);
    }
}
