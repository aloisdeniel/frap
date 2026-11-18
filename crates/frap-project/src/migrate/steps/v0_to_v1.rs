// SPDX-License-Identifier: GPL-3.0-or-later
//! Schema 0 → 1.
//!
//! Schema 0 is the pre-1.0 draft layout (`docs/design/proposal-buildable.md` §16), which
//! differs from §15 of ARCHITECTURE.md in two keys:
//! - `manifest.json`: the Blender API version was `blender_rna_version`; now `blender_api`.
//! - `scene.json`: a mesh attribute's type was `type`; now `data_type`, the RNA identifier
//!   (`Attribute.data_type`), as every key of `scene.json` is.

use crate::migrate::{Migration, MigrationCx, for_each_id, rename_key};

pub const MIGRATION: Migration = Migration {
    from: 0,
    summary: "renamed manifest `blender_rna_version` to `blender_api` and mesh attribute `type` to `data_type`",
    apply,
};

fn apply(cx: &mut MigrationCx<'_>) -> Result<(), String> {
    rename_key(cx.manifest, "blender_rna_version", "blender_api")?;
    for_each_id(cx.scene, "meshes", |name, mesh| {
        let Some(attrs) = mesh.get_mut("attributes") else {
            return Ok(());
        };
        let attrs = attrs
            .as_array_mut()
            .ok_or_else(|| format!("meshes[{name:?}].attributes is not an array"))?;
        for attr in attrs {
            let attr = attr
                .as_object_mut()
                .ok_or_else(|| format!("meshes[{name:?}] has a non-object attribute"))?;
            rename_key(attr, "type", "data_type")
                .map_err(|e| format!("meshes[{name:?}] attribute: {e}"))?;
        }
        Ok(())
    })?;
    Ok(())
}
