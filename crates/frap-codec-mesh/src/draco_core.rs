// SPDX-License-Identifier: GPL-3.0-or-later
//! The draco-core backend (feature `draco`, WP 7.2; M7 W0 stub).

use crate::draco::DracoCodec;

/// The draco-core codec, `None` until WP 7.2 implements it (or without the `draco` feature).
pub fn codec() -> Option<&'static dyn DracoCodec> {
    None
}
