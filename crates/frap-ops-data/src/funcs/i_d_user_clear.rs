// SPDX-License-Identifier: GPL-3.0-or-later
//! `ID.user_clear()` (`rna_ID_user_clear`): Blender sets the user count to 0 (so a save drops
//! the ID) and clears `use_fake_user`, without unlinking anything (verified on 5.2.4: an
//! object still in its collection reads `users == 0`).
//!
//! frap computes `ID.users` from the references on demand (§5.5) and has no stored count to
//! zero: it clears `use_fake_user` and, when references remain, says so with a WARNING
//! (deviation: `users` keeps counting those references).

use frap_rna::{Report, ReportType, RnaError, RnaPtr, Root, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_common::{clear_fake_user, internal};

#[rna_func(srna = "ID", func = "user_clear")]
fn i_d_user_clear(
    h: &mut dyn Host,
    this: &RnaPtr,
    _args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    super::_common::refuse_detached(this, "user_clear")?;
    let a = match this.root {
        Root::Id(a) => a,
        // Embedded IDs have no user count of their own.
        Root::Embedded { .. } => return Ok(Value::None),
        _ => return Err(internal("ID.user_clear(): not a datablock")),
    };
    let (main, log) = h.main_mut();
    if !clear_fake_user(main, a, log) {
        return Err(internal("ID.user_clear(): the datablock was removed"));
    }
    let users = main.users(a);
    if users > 0 {
        let name = main.header(a).map(|h| h.name.clone()).unwrap_or_default();
        h.report(Report::new(
            ReportType::Warning,
            format!(
                "frap: ID.user_clear(): '{name}' is still referenced ({users} user{}); frap \
                 counts users from references, so only use_fake_user was cleared",
                if users == 1 { "" } else { "s" }
            ),
        ));
    }
    Ok(Value::None)
}
