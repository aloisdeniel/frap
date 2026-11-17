// SPDX-License-Identifier: GPL-3.0-or-later
//! `ID.evaluated_get(depsgraph)` (`rna_ID_evaluated_get` → `DEG_get_evaluated_id`): an
//! evaluation boundary (§7.3: pending animation is applied and the depsgraph-lite updated),
//! then the evaluated ID.
//!
//! Verified on 5.2.4: Blender returns a separate evaluated copy (`is_evaluated == True`,
//! `ev == ob` False, `ev.original == ob`, repr `<bpy_struct, Object("Cube") at 0x…,
//! evaluated>`) for every ID the depsgraph evaluates (the scene, its collections, objects,
//! their data and materials, the world, a material's node tree); an ID outside it (an unused
//! material or mesh, an object in no scene) is returned as the original. Writes to the copy
//! leave the original untouched (`ev.location.x = 7` keeps `ob.location.x`).
//!
//! frap has no evaluated IDs of its own (M4): it returns the original behind a
//! [`Detached::Evaluated`] pointer, whose reads are the original's always-fresh values (D-04:
//! `matrix_world`, `dimensions`, `to_mesh()`), whose `is_evaluated` reads True and `original`
//! leads back, and through which the session refuses writes (deviation: Blender accepts them
//! and changes only the copy; deviations.md).

use frap_rna::{Detached, IdCode, RnaError, RnaPtr, Root, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_common::{arg_ptr, detached, in_depsgraph, internal};

#[rna_func(srna = "ID", func = "evaluated_get")]
fn i_d_evaluated_get(
    h: &mut dyn Host,
    this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    h.apply_pending_animation()?;
    let (host, ev) = h.eval();
    ev.depsgraph_update(host);
    // The depsgraph is `scene.view_layers[…].depsgraph`, rooted at its scene.
    let scene = match arg_ptr(args, "depsgraph").map(|p| p.root) {
        Some(Root::Id(a)) if a.code == IdCode::SC => a.typed(),
        _ => None,
    }
    .unwrap_or(h.win().scene);
    let id = match this.root {
        Root::Id(a) if this.path.is_empty() => a,
        Root::Embedded { owner, .. } if this.path.is_empty() => owner,
        _ => return Err(internal("ID.evaluated_get(): not a datablock")),
    };
    let original = RnaPtr {
        detached: None,
        ..this.clone()
    };
    if this.detached == Some(Detached::Evaluated)
        || in_depsgraph(h.main(), scene, id)
    {
        Ok(Value::Ptr(Box::new(detached(original, Detached::Evaluated))))
    } else {
        Ok(Value::Ptr(Box::new(original)))
    }
}
