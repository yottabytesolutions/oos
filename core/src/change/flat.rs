// SPDX-License-Identifier: GPL-2.0-or-later

use super::storage::refresh;
use super::{Classes as C, FieldDiff};
#[cfg(test)]
use crate::model::AutoAcquisition;
use crate::model::{Model, TrigMode, TrigType};

macro_rules! classes {
    ($a:ident, $b:ident; decoder) => {
        if $a.trig_type == TrigType::Bus || $b.trig_type == TrigType::Bus { C::ACQ | C::STATS } else { C::NONE }
    };
    ($a:ident, $b:ident; sweep) => {
        if $a.run == $b.run && $b.trig_mode != TrigMode::Single { C::ACQ | C::STATS } else { C::NONE }
    };
    ($a:ident, $b:ident; $($class:ident)|+) => { $(C::$class)|+ };
}

macro_rules! define_flat {
    (nested { $($nested:ident: $nested_ty:ident { $($member:ident),* })* }
    $($(#[$attribute:meta])* $group:ident $module:ident {
        $($index:literal $variant:ident($($root:ident)? $([$parent:ident] $field:ident)? : $ty:ty)
            $policy:ident [$($class:tt)*];)*
    })*) => {
        pub(super) fn fields(a: &Model, b: &Model, mut emit: impl FnMut(FieldDiff)) {
            let mut channels = None;
            $($(let diff = FieldDiff::new(
                $index,
                stringify!($($root)?$($field)?),
                a.$($root)?$($parent.$field)? != b.$($root)?$($parent.$field)?,
                classes!(a, b; $($class)*)
            );
                if $index == 0 { channels = Some(diff); } else { emit(diff); }
            )*)*
            // The whole-channel slot has always followed the other whole fields.
            if let Some(diff) = channels { emit(diff); }
        }
        pub(super) fn refresh(a: &mut Model, b: &Model) {
            $($(refresh!($policy, &mut a.$($root)?$($parent.$field)?, &b.$($root)?$($parent.$field)?);)*)*
        }
        #[cfg(test)]
        pub(super) const ENTRIES: &[(usize, &str)] = &[$($(($index, stringify!($($root)?$($field)?)),)*)*];
        #[cfg(test)]
        pub(super) fn assert_exhaustive(m: &Model) {
            let Model { $($($($root: _,)?)*)* $($nested: $nested_ty { $($member: _,)* },)* } = m;
        }
    };
}
super::field_table::field_table!(define_flat);
