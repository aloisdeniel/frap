// SPDX-License-Identifier: GPL-3.0-or-later
//! The operator universe of `--describe`: every operator Blender 5.2 has (the catalog plus the
//! typed specs of every `frap-spec-ops-*` group) and frap's own `frap.*` operators, each paired
//! with its registered `OperatorDef` when frap implements it. Built once per process from the
//! generated tables and the `OPERATORS` slice, so it cannot drift from the registry.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use frap_op::{OPERATORS, OperatorDef};
use frap_rna::OperatorSpec;

/// One operator of the universe.
#[derive(Copy, Clone)]
pub struct OpEntry {
    /// Dumped (or hand-written `frap.*`) spec. For an implemented operator this is the
    /// registered `OperatorDef::spec`.
    pub spec: &'static OperatorSpec,
    /// The registered implementation, if any.
    pub def: Option<&'static OperatorDef>,
}

impl std::fmt::Debug for OpEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpEntry")
            .field("idname", &self.spec.idname)
            .field("implemented", &self.def.is_some())
            .finish()
    }
}

impl OpEntry {
    /// `ns` of `ns.op`.
    pub fn namespace(&self) -> &'static str {
        self.spec.idname.split_once('.').map_or("", |(ns, _)| ns)
    }
    /// `op` of `ns.op`.
    pub fn name(&self) -> &'static str {
        self.spec
            .idname
            .split_once('.')
            .map_or(self.spec.idname, |(_, op)| op)
    }
    /// True when frap registers an implementation.
    pub fn implemented(&self) -> bool {
        self.def.is_some()
    }
}

/// Every typed spec of every ops group (OPERATORS.md milestone ≤ current).
fn typed_specs() -> impl Iterator<Item = &'static OperatorSpec> {
    let groups: [&'static [&'static OperatorSpec]; 9] = [
        &frap_spec_ops_object::ALL_OPS,
        &frap_spec_ops_transform::ALL_OPS,
        &frap_spec_ops_mesh::ALL_OPS,
        &frap_spec_ops_uv::ALL_OPS,
        &frap_spec_ops_anim::ALL_OPS,
        &frap_spec_ops_data::ALL_OPS,
        &frap_spec_ops_io::ALL_OPS,
        &frap_spec_ops_render::ALL_OPS,
        &frap_spec_ops_frap::ALL_OPS,
    ];
    groups.into_iter().flat_map(|g| g.iter().copied())
}

fn build() -> Vec<OpEntry> {
    let mut by: BTreeMap<&'static str, OpEntry> = BTreeMap::new();
    for spec in frap_spec_catalog::catalog()
        .iter()
        .copied()
        .chain(typed_specs())
    {
        by.entry(spec.idname).or_insert(OpEntry { spec, def: None });
    }
    // The registry wins: its spec is what the pipeline converts arguments against. A
    // duplicate registration is the registry's error (`Registry::global` panics); here the
    // first one is kept so `--describe` still works.
    for def in OPERATORS.iter() {
        let e = by.entry(def.spec.idname).or_insert(OpEntry {
            spec: def.spec,
            def: None,
        });
        if e.def.is_none() {
            *e = OpEntry {
                spec: def.spec,
                def: Some(def),
            };
        }
    }
    by.into_values().collect()
}

/// Every operator, sorted by idname. Built on first use.
pub fn all_ops() -> &'static [OpEntry] {
    static ALL: OnceLock<Vec<OpEntry>> = OnceLock::new();
    ALL.get_or_init(build)
}

/// One operator by `ns.op` idname (also accepts `bpy.ops.ns.op` and `NS_OT_op`).
pub fn find_op(name: &str) -> Option<&'static OpEntry> {
    let name = name.strip_prefix("bpy.ops.").unwrap_or(name);
    let all = all_ops();
    if let Ok(i) = all.binary_search_by(|e| e.spec.idname.cmp(name)) {
        return Some(&all[i]);
    }
    all.iter().find(|e| e.spec.bl_idname == name)
}

/// Namespaces of the universe, sorted.
pub fn namespaces() -> Vec<&'static str> {
    let mut v: Vec<&'static str> = all_ops().iter().map(OpEntry::namespace).collect();
    v.dedup();
    v
}

/// Glob match (`*` any run, `?` one char) of `pattern` against `text`; several patterns may be
/// given separated by commas (`mesh.*,object.select_*`).
pub fn glob_match(patterns: &str, text: &str) -> bool {
    patterns
        .split(',')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .any(|p| glob1(p.as_bytes(), text.as_bytes()))
}

fn glob1(p: &[u8], t: &[u8]) -> bool {
    // Iterative wildcard matching with backtracking to the last '*'.
    let (mut pi, mut ti) = (0, 0);
    let mut star: Option<(usize, usize)> = None;
    while ti < t.len() {
        if pi < p.len() && (p[pi] == b'?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == b'*' {
            star = Some((pi, ti));
            pi += 1;
        } else if let Some((sp, st)) = star {
            pi = sp + 1;
            ti = st + 1;
            star = Some((sp, st + 1));
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == b'*' {
        pi += 1;
    }
    pi == p.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn globs() {
        assert!(glob_match("mesh.*", "mesh.bevel"));
        assert!(!glob_match("mesh.*", "object.delete"));
        assert!(glob_match("object.select_*,mesh.bevel", "mesh.bevel"));
        assert!(glob_match("*.primitive_*_add", "mesh.primitive_cube_add"));
        assert!(glob_match("mesh.bev?l", "mesh.bevel"));
        assert!(!glob_match("mesh.bev?l", "mesh.bevelx"));
        assert!(glob_match("*", "x"));
    }

    #[test]
    fn universe_is_the_catalog_plus_the_typed_specs() {
        let n = all_ops().len();
        assert_eq!(
            n,
            frap_spec_catalog::generated::CATALOG_LEN
                + frap_spec_catalog::generated::TYPED_IDNAMES.len()
        );
        assert!(
            all_ops()
                .windows(2)
                .all(|w| w[0].spec.idname < w[1].spec.idname)
        );
        assert_eq!(
            find_op("bpy.ops.mesh.bevel").map(|e| e.spec.bl_idname),
            Some("MESH_OT_bevel")
        );
        assert_eq!(
            find_op("MESH_OT_bevel").map(|e| e.spec.idname),
            Some("mesh.bevel")
        );
        assert!(find_op("mesh.nope").is_none());
    }
}
