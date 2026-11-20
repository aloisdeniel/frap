// SPDX-License-Identifier: GPL-3.0-or-later
//! `ShapeKey.name` (`rna_ShapeKey_name_set` → `BKE_keyblock_rename`, WP 6.3): unique among the
//! key's blocks (`BLI_uniquename`, default `Key`, 63 bytes), then the animation paths that
//! named the old block (`key_blocks["Old"]…`) are renamed
//! (`BKE_animdata_fix_paths_rename_all(nullptr, "key_blocks", old, new)`). frap renames the
//! F-curves of every action (Blender visits the actions assigned to an ID; an unassigned
//! action keeps its paths there).

use frap_rna::ptr::rna_key_repr;
use frap_rna::{PropDef, RnaError, RnaPtr, Value, rna_hook};

use crate::Host;
use crate::id::Key;
use crate::id::paths::internal;

#[rna_hook(srna = "ShapeKey", prop = "name", krate = crate)]
fn set(h: &mut dyn Host, this: &RnaPtr, _prop: &'static PropDef, v: Value) -> Result<(), RnaError> {
    let (k, i) = Key::block_of(h.main(), this)
        .ok_or_else(|| internal("ShapeKey.name: not a shape key pointer"))?;
    let requested = v.as_str().unwrap_or_default().to_owned();
    let (main, log) = h.main_mut();
    let Some(key) = main.shape_keys.get_mut(k, log) else {
        return Err(internal("ShapeKey.name: the key was removed"));
    };
    let old = key.key_blocks[i].name.clone();
    let new = key.rename_block(i, &requested);
    if old == new {
        return Ok(());
    }
    let (from, to) = (
        format!("key_blocks{}", rna_key_repr(&old)),
        format!("key_blocks{}", rna_key_repr(&new)),
    );
    let hits: Vec<_> = main
        .actions
        .iter()
        .filter(|(_, a)| {
            a.layers.iter().any(|l| {
                l.strips.iter().any(|s| match s {
                    frap_anim::ActionStrip::Keyframe(ks) => ks.channelbags.iter().any(|cb| {
                        cb.fcurves
                            .iter()
                            .any(|f| rename_path(&f.data_path, &from, &to).is_some())
                    }),
                    _ => false,
                })
            })
        })
        .map(|(r, _)| r)
        .collect();
    for r in hits {
        if let Some(a) = main.actions.get_mut(r, log) {
            for l in &mut a.layers {
                for s in &mut l.strips {
                    let frap_anim::ActionStrip::Keyframe(ks) = s else {
                        continue;
                    };
                    for cb in &mut ks.channelbags {
                        for f in &mut cb.fcurves {
                            if let Some(p) = rename_path(&f.data_path, &from, &to) {
                                f.data_path = p.into();
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

/// `from…` → `to…` when `path` starts with `from` followed by the end or `.`/`[`.
fn rename_path(path: &str, from: &str, to: &str) -> Option<String> {
    let rest = path.strip_prefix(from)?;
    (rest.is_empty() || rest.starts_with('.') || rest.starts_with('[')).then(|| format!("{to}{rest}"))
}

#[cfg(test)]
mod tests {
    use super::rename_path;

    #[test]
    fn renames_only_whole_block_paths() {
        let (f, t) = (r#"key_blocks["A"]"#, r#"key_blocks["B"]"#);
        assert_eq!(
            rename_path(r#"key_blocks["A"].value"#, f, t).as_deref(),
            Some(r#"key_blocks["B"].value"#)
        );
        assert_eq!(rename_path(r#"key_blocks["AB"].value"#, f, t), None);
    }
}
