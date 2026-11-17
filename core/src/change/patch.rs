// SPDX-License-Identifier: GPL-2.0-or-later

//! Bounded, typed flat-model proposals for the transitional internal outbox.
use super::storage::{apply, bytes, capture, refresh, reuse};
use crate::math::{MathOp, Window};
use crate::model::*;

macro_rules! define_patch {
    ($($(#[$attribute:meta])* $group:ident $module:ident {
        $($index:literal $variant:ident($field:ident: $ty:ty) $policy:ident [$($class:tt)*];)*
    })*) => {
        $(mod $module {
            use super::*;
            $(#[$attribute])*
            #[derive(Clone, Debug, PartialEq)]
            pub enum $group { $($variant($ty),)* }
            impl $group {
                pub(super) fn apply(&self, m: &mut Model) {
                    match self { $(Self::$variant(value) => {
                        apply!($policy, &mut m.$field, value);
                    },)* }
                }
                pub(super) fn reuse(&mut self, m: &Model) {
                    match self { $(Self::$variant(value) => {
                        reuse!($policy, value, &m.$field);
                    },)* }
                }
                pub(super) fn bytes(&self) -> usize {
                    match self { $(Self::$variant(_value) => bytes!($policy, _value),)* }
                }
            }
        })*
        #[derive(Clone, Debug, PartialEq)]
        pub enum Patch { $($group($module::$group),)* }
        impl Patch {
            pub(super) fn capture(index: usize, m: &Model) -> Option<Self> {
                Some(match index {
                    $($($index => Self::$group($module::$group::$variant(capture!($policy, &m.$field))),)*)*
                    _ => return None,
                })
            }
            pub(super) fn index(&self) -> usize {
                match self { $($(Self::$group($module::$group::$variant(_)) => $index,)*)* }
            }
            pub(super) fn apply(&self, m: &mut Model) {
                match self { $(Self::$group(p) => p.apply(m),)* }
            }
            pub(super) fn reuse(&mut self, m: &Model) {
                match self { $(Self::$group(p) => p.reuse(m),)* }
            }
            pub(super) fn bytes(&self) -> usize {
                match self { $(Self::$group(p) => p.bytes(),)* }
            }
        }
    };
}
super::field_table::field_table!(define_patch);
