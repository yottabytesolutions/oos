// SPDX-License-Identifier: GPL-2.0-or-later

//! Storage policies shared by generated patches and snapshot refresh.

macro_rules! capture {
    (copy, $value:expr) => {
        *$value
    };
    (boxed_copy, $value:expr) => {
        Box::new(*$value)
    };
    (boxed_clone, $value:expr) => {
        Box::new($value.clone())
    };
    ($policy:ident, $value:expr) => {
        $value.clone()
    };
}
pub(super) use capture;

macro_rules! refresh {
    (strings, $a:expr, $b:expr) => {
        for (a, b) in $a.iter_mut().zip($b) {
            a.clone_from(b);
        }
    };
    (zones, $a:expr, $b:expr) => {
        for (a, b) in $a.iter_mut().zip($b) {
            a.on = b.on;
            a.src = b.src;
            a.inside = b.inside;
            a.pts.clone_from(&b.pts);
        }
    };
    ($policy:ident, $a:expr, $b:expr) => {
        $a.clone_from($b)
    };
}
pub(super) use refresh;

macro_rules! bytes {
    (strings, $value:expr) => {
        $value.iter().map(String::capacity).sum()
    };
    (zones, $value:expr) => {
        $value.iter().map(|z| z.pts.capacity() * std::mem::size_of::<(f64, f64)>()).sum()
    };
    (bus, $value:expr) => {
        $value.label.capacity()
    };
    (vec, $value:expr) => {
        $crate::change::storage::vec_bytes($value)
    };
    (string, $value:expr) => {
        $value.capacity()
    };
    (boxed_copy, $value:expr) => {
        std::mem::size_of_val($value.as_ref())
    };
    (boxed_clone, $value:expr) => {
        std::mem::size_of_val($value.as_ref())
    };
    ($policy:ident, $value:expr) => {
        0
    };
}
pub(super) use bytes;

pub(super) fn vec_bytes<T>(value: &Vec<T>) -> usize {
    value.capacity() * std::mem::size_of::<T>()
}

macro_rules! apply {
    (boxed_copy, $a:expr, $b:expr) => {
        $a.clone_from($b.as_ref())
    };
    (boxed_clone, $a:expr, $b:expr) => {
        $a.clone_from($b.as_ref())
    };
    ($policy:ident, $a:expr, $b:expr) => {
        refresh!($policy, $a, $b)
    };
}
pub(super) use apply;

macro_rules! reuse {
    (boxed_copy, $a:expr, $b:expr) => {
        $a.as_mut().clone_from($b)
    };
    (boxed_clone, $a:expr, $b:expr) => {
        $a.as_mut().clone_from($b)
    };
    ($policy:ident, $a:expr, $b:expr) => {
        refresh!($policy, $a, $b)
    };
}
pub(super) use reuse;
