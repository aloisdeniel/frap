// SPDX-License-Identifier: GPL-3.0-or-later
//! Schema migrations, one file per step (`v{N}_to_v{N+1}.rs`), on `serde_json::Value` before
//! typed deserialization; newer versions are refused (§15). WP 1.9.
//!
//! This is frap's counterpart of Blender's `versioning_*.cc`: a file saved with
//! `schema_version` N is brought to [`SCHEMA_VERSION`] by running the steps N→N+1, N+1→N+2, …
//! on the parsed `manifest.json` and `scene.json` trees. Each step only rewrites JSON; the
//! typed `scene.json` mapping (WP 1.23) only ever sees the current schema.
//!
//! Adding a schema version: add `steps/v{N}_to_v{N+1}.rs` exporting `pub const MIGRATION`,
//! append it to [`MIGRATIONS`], and bump [`SCHEMA_VERSION`]. The `migrations_cover_every_step_file`
//! test fails if a step file is not registered or the chain has a gap.

use serde_json::{Map, Value};

pub use crate::manifest::SCHEMA_VERSION;

/// Migration steps (automod: adding a step needs no `mod` edit).
pub mod steps {
    automod::dir!(pub "src/migrate/steps");
}

/// Errors of version checks and migrations.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MigrateError {
    #[error(
        "file written by a newer frap (schema_version {found}); this build reads schema_version {supported} and older"
    )]
    Newer { found: u32, supported: u32 },
    #[error("no migration from schema_version {0}")]
    NoStep(u32),
    #[error("migrating schema_version {from} to {}: {message}", from + 1)]
    Failed { from: u32, message: String },
}

/// The trees one step rewrites.
#[derive(Debug)]
pub struct MigrationCx<'a> {
    /// `manifest.json` (its `schema_version` is updated by the framework after each step).
    pub manifest: &'a mut Map<String, Value>,
    /// `scene.json`.
    pub scene: &'a mut Value,
}

/// One schema step `from` → `from + 1`.
#[derive(Clone, Copy, Debug)]
pub struct Migration {
    pub from: u32,
    /// One line describing what changed, reported to the user after a migrated load.
    pub summary: &'static str,
    pub apply: fn(&mut MigrationCx<'_>) -> Result<(), String>,
}

/// Every step, in order: `MIGRATIONS[i].from == i`, and `MIGRATIONS.len() == SCHEMA_VERSION`.
pub static MIGRATIONS: &[Migration] = &[steps::v0_to_v1::MIGRATION];

/// What a load migrated.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MigrationReport {
    pub from: u32,
    pub to: u32,
    /// `summary` of every step applied, in order.
    pub applied: Vec<&'static str>,
}

/// Refuses files newer than this build.
pub fn check_supported(found: u32) -> Result<(), MigrateError> {
    if found > SCHEMA_VERSION {
        Err(MigrateError::Newer {
            found,
            supported: SCHEMA_VERSION,
        })
    } else {
        Ok(())
    }
}

/// Brings `manifest` and `scene` (from a file with `from`'s schema) to [`SCHEMA_VERSION`].
/// Returns `None` when the file was already current.
pub fn migrate(
    from: u32,
    manifest: &mut Map<String, Value>,
    scene: &mut Value,
) -> Result<Option<MigrationReport>, MigrateError> {
    migrate_with(MIGRATIONS, SCHEMA_VERSION, from, manifest, scene)
}

/// [`migrate`] over an explicit step table (tests use synthetic chains).
pub fn migrate_with(
    steps: &[Migration],
    target: u32,
    from: u32,
    manifest: &mut Map<String, Value>,
    scene: &mut Value,
) -> Result<Option<MigrationReport>, MigrateError> {
    if from > target {
        return Err(MigrateError::Newer {
            found: from,
            supported: target,
        });
    }
    if from == target {
        return Ok(None);
    }
    let mut applied = Vec::new();
    for version in from..target {
        let step = steps
            .iter()
            .find(|s| s.from == version)
            .ok_or(MigrateError::NoStep(version))?;
        (step.apply)(&mut MigrationCx { manifest, scene }).map_err(|message| {
            MigrateError::Failed {
                from: version,
                message,
            }
        })?;
        manifest.insert("schema_version".into(), Value::from(version + 1));
        applied.push(step.summary);
    }
    Ok(Some(MigrationReport {
        from,
        to: target,
        applied,
    }))
}

/// Renames `old` to `new` in `obj`, keeping the key's position (written key order is
/// significant for diffs). Returns whether `old` was present. Fails if both exist.
pub fn rename_key(obj: &mut Map<String, Value>, old: &str, new: &str) -> Result<bool, String> {
    if !obj.contains_key(old) {
        return Ok(false);
    }
    if obj.contains_key(new) {
        return Err(format!("both `{old}` and `{new}` are present"));
    }
    let entries = std::mem::take(obj);
    for (k, v) in entries {
        if k == old {
            obj.insert(new.to_owned(), v);
        } else {
            obj.insert(k, v);
        }
    }
    Ok(true)
}

/// Calls `f(name, id)` for every ID of a `scene.json` ID collection (`"meshes"`, …), local
/// and linked (`{"linked": {"<library>": {name: id}}}`, §15).
pub fn for_each_id(
    scene: &mut Value,
    collection: &str,
    mut f: impl FnMut(&str, &mut Value) -> Result<(), String>,
) -> Result<(), String> {
    let Some(coll) = scene.get_mut(collection) else {
        return Ok(());
    };
    let coll = coll
        .as_object_mut()
        .ok_or_else(|| format!("`{collection}` is not an object"))?;
    for (name, id) in coll.iter_mut() {
        if name == "linked" {
            let libs = id
                .as_object_mut()
                .ok_or_else(|| format!("`{collection}.linked` is not an object"))?;
            for (lib, ids) in libs.iter_mut() {
                let ids = ids
                    .as_object_mut()
                    .ok_or_else(|| format!("`{collection}.linked.{lib}` is not an object"))?;
                for (name, id) in ids.iter_mut() {
                    f(name, id)?;
                }
            }
        } else {
            f(name, id)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
