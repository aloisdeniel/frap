// SPDX-License-Identifier: GPL-3.0-or-later
//! Explained mismatches: differences the differential fuzzer finds that `docs/deviations.md`
//! records (a deliberate deviation of §1 or a temporary gap of §2), so a run reports them as
//! explained instead of unexplained (WP 8.7).
//!
//! Matching: for a `lines` mismatch the first statement pattern must occur in the statement
//! of that line and the others anywhere in the sequence; for other comparators every pattern
//! must occur somewhere in the sequence. The comparator and a needle (a substring of the
//! mismatch path or detail) narrow the entry. A `cascade` entry (an unimplemented case that
//! leaves the scene different from there on) explains every other mismatch of its trial.
//!
//! Entries are found by fuzzing and triaged by hand: a bug is fixed by the owner of the code
//! (its minimised case then joins `tests/golden/cases/fuzz/`); a documented difference gets
//! an entry here naming its `docs/deviations.md` row (checked by a test).

use crate::case::Mismatch;

/// One explained mismatch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Known {
    /// The `docs/deviations.md` row: `D-nn`, or `§2 <words of the row>`.
    pub deviation: &'static str,
    /// Statement substrings (see the module docs).
    pub statements: &'static [&'static str],
    /// The comparator (`lines`, `rna_values`, `mesh_canonical`, …), `*` for any.
    pub comparator: &'static str,
    /// A substring of the mismatch path or detail, `""` for any.
    pub needle: &'static str,
    /// Explains the rest of the trial too.
    pub cascade: bool,
}

const fn gap(
    deviation: &'static str,
    statements: &'static [&'static str],
    needle: &'static str,
) -> Known {
    Known {
        deviation,
        statements,
        comparator: "lines",
        needle,
        cascade: true,
    }
}

const fn diff(
    deviation: &'static str,
    statements: &'static [&'static str],
    comparator: &'static str,
    needle: &'static str,
) -> Known {
    Known {
        deviation,
        statements,
        comparator,
        needle,
        cascade: false,
    }
}

const D_74: &str = "D-74";
const D_111: &str = "D-111";
const POLL: &str = "poll() failed";
const INCORRECT: &str = "context is incorrect";
const MESH: &str = "mesh_canonical";
const TORUS: &str = "§2 `mesh.primitive_torus_add` while a mesh is in Edit Mode";
const QSORT: &str = "§2 Ties in the C library `qsort` outside `uv.smart_project`";
const SNAP: &str =
    "§2 Differential fuzzing (WP 8.7), unimplemented modes: `view3d.snap_cursor_to_selected`";
const TRANSFORM_OBJECT: &str =
    "§2 Differential fuzzing (WP 8.7), unimplemented modes: `transform.mirror`";
const POLLS: &str = "§2 Differential fuzzing (WP 8.7), operator polls and outcomes";
const GEOMETRY: &str = "§2 Differential fuzzing (WP 8.7), geometry differences";
const UV: &str = "§2 Differential fuzzing (WP 8.7), UV results";

const fn cascade(
    deviation: &'static str,
    statements: &'static [&'static str],
    comparator: &'static str,
    needle: &'static str,
) -> Known {
    Known {
        deviation,
        statements,
        comparator,
        needle,
        cascade: true,
    }
}

/// The explained mismatches (see the module docs).
pub const KNOWN: &[Known] = &[
    // The startup Cube (and new meshes) store no active attribute.
    cascade(D_74, &["geometry.attribute_remove"], "lines", ""),
    cascade(D_74, &["geometry.attribute_convert"], "lines", ""),
    gap(TORUS, &["mesh.primitive_torus_add"], "not implemented"),
    // UV-editor-only selection operators run as in an Image Editor (headless-extended).
    diff(D_111, &["uv.select_more"], "lines", POLL),
    diff(D_111, &["uv.select_less"], "lines", POLL),
    diff(D_111, &["uv.select_mode"], "lines", POLL),
    diff(D_111, &["uv.select_similar"], "lines", POLL),
    diff(D_111, &["uv.shortest_path_select"], "lines", POLL),
    // The box packer's qsort ties.
    diff(QSORT, &["uv.lightmap_pack"], MESH, "uv_layers"),
    gap(SNAP, &["bpy.ops.view3d.snap_"], "is not implemented yet"),
    gap(
        TRANSFORM_OBJECT,
        &["bpy.ops.transform."],
        "in OBJECT mode yet",
    ),
    cascade(POLLS, &["object.modifiers_clear"], "lines", INCORRECT),
    diff(POLLS, &["object.parent_no_inverse_set"], "lines", INCORRECT),
    // Edit Mode on a hidden object: the first pattern is any statement.
    cascade(
        POLLS,
        &["bpy.ops.", "hide_view_set", "mode_set(mode='EDIT')"],
        "*",
        "",
    ),
    cascade(
        POLLS,
        &["object.vertex_group_normalize_all"],
        "lines",
        "locked",
    ),
    cascade(POLLS, &["bpy.ops.", "uv.hide"], "*", ""),
    cascade(POLLS, &["object.modifier_apply"], "lines", "disabled"),
    diff(
        POLLS,
        &["object.vertex_group_normalize"],
        "lines",
        "CANCELLED",
    ),
    diff(POLLS, &["object.align"], "lines", "UnboundLocalError"),
    cascade(POLLS, &["uv.lightmap_pack"], "rna_values", "mode"),
    diff(GEOMETRY, &["mesh.loop_to_region"], MESH, ""),
    diff(GEOMETRY, &["mesh.flatten"], MESH, ""),
    diff(
        GEOMETRY,
        &["mesh.vert_connect_nonplanar"],
        MESH,
        "uv_layers",
    ),
    diff(GEOMETRY, &["mesh.extrude_edges_"], MESH, ""),
    diff(GEOMETRY, &["mesh.extrude_context"], MESH, "edges"),
    cascade(GEOMETRY, &["ORIGIN_CENTER_OF_VOLUME"], "rna_values", ""),
    diff(GEOMETRY, &["mode_set(mode='EDIT')"], MESH, "evaluated["),
    diff(
        GEOMETRY,
        &["shape_key_add", "transform_apply"],
        MESH,
        "evaluated[",
    ),
    diff(GEOMETRY, &["mesh.average_normals"], MESH, "custom_normal"),
    diff(
        GEOMETRY,
        &["mesh.normals_tools", "mesh.dissolve_"],
        MESH,
        "normal",
    ),
    diff(GEOMETRY, &["mesh.edge_face_add"], MESH, ""),
    diff(GEOMETRY, &["mesh.decimate"], MESH, ""),
    diff(GEOMETRY, &["uv.align"], MESH, "uv_layers"),
    diff(GEOMETRY, &["mesh.fill("], MESH, ""),
    cascade(
        POLLS,
        &["object.vertex_group_copy", "object.modifier_apply"],
        "lines",
        "",
    ),
    cascade(POLLS, &["transform.", "shape_key_lock"], "lines", "locked"),
    diff(GEOMETRY, &["SUBSURF", "BEVEL"], MESH, "evaluated["),
    diff(
        GEOMETRY,
        &["mesh.edge_split", "mesh.merge_normals"],
        MESH,
        "normals",
    ),
    diff(GEOMETRY, &["mesh.edge_rotate", "TRIFAN"], MESH, ""),
    diff(
        GEOMETRY,
        &["normals_make_consistent", "primitive_plane_add"],
        MESH,
        "",
    ),
    diff(
        GEOMETRY,
        &["shape_key_add", "mode_set(mode='EDIT')", "primitive_"],
        MESH,
        "",
    ),
    diff(GEOMETRY, &["uv.weld", "mesh.split"], MESH, "uv_layers"),
    diff(GEOMETRY, &["mesh.convex_hull", "uv.select_"], MESH, ""),
    diff(
        GEOMETRY,
        &["mesh.fill_grid", "material_slot_add"],
        MESH,
        "material_index",
    ),
    diff(GEOMETRY, &["quads_convert_to_tris"], MESH, ""),
    diff(GEOMETRY, &["shape_key_add", "transform_apply"], MESH, ""),
    diff(
        GEOMETRY,
        &["parent_set", "view3d.snap_selected_to_cursor"],
        "rna_values",
        "",
    ),
    diff(GEOMETRY, &["use_proportional_edit=True"], MESH, "normals"),
    diff(GEOMETRY, &["mesh.spin"], MESH, "normals"),
    diff(GEOMETRY, &["mesh.smooth_normals"], MESH, "normal"),
    diff(GEOMETRY, &["mesh.average_normals"], MESH, "normal"),
    diff(GEOMETRY, &["mesh.merge_normals"], MESH, "normal"),
    diff(GEOMETRY, &["mesh.split_normals"], MESH, "normal"),
    diff(GEOMETRY, &["mesh.set_normals_from_faces"], MESH, "normal"),
    diff(POLLS, &["object.transform_apply"], "lines", "aborting"),
    diff(GEOMETRY, &["subdivision_set", "BEVEL"], MESH, "evaluated["),
    diff(
        GEOMETRY,
        &["mesh.customdata_custom_splitnormals_add"],
        MESH,
        "normal",
    ),
    diff(
        GEOMETRY,
        &["shape_key_add", "mode_set(mode='EDIT')"],
        MESH,
        "positions",
    ),
    diff(GEOMETRY, &["transform.mirror"], MESH, ""),
    diff(GEOMETRY, &["mesh.bisect"], MESH, ""),
    diff(GEOMETRY, &["mesh.dissolve_limited"], MESH, ""),
    diff(GEOMETRY, &["mesh.delete_edgeloop"], MESH, ""),
    diff(
        GEOMETRY,
        &["mesh.dissolve_verts", "mesh.edge_split"],
        MESH,
        "",
    ),
    diff(GEOMETRY, &["transform.vertex_random"], MESH, "positions"),
    diff(
        GEOMETRY,
        &["mesh.extrude_faces_indiv", "transform.shrink_fatten"],
        MESH,
        "positions",
    ),
    diff(GEOMETRY, &["orient_type='NORMAL'"], MESH, "positions"),
    // Blender's own UV sphere element order changes between runs: index writes are not
    // comparable after one, nor results that depend on the element order.
    cascade(
        "D-19",
        &["bpy.", "primitive_uv_sphere_add", ".data."],
        "*",
        "",
    ),
    diff("D-19", &["primitive_uv_sphere_add"], MESH, ""),
    diff(UV, &["uv.unwrap"], MESH, "uv_layers"),
    diff(UV, &["uv.smart_project"], MESH, "uv_layers"),
    diff(UV, &["uv.sphere_project"], MESH, "uv_layers"),
    diff(UV, &["uv.cube_project"], MESH, "uv_layers"),
    diff(UV, &["uv.cylinder_project"], MESH, "uv_layers"),
    diff(UV, &["uv.remove_doubles"], MESH, "uv_layers"),
    diff(UV, &["uv.weld"], MESH, "uv_layers"),
    diff(UV, &["uv.average_islands_scale"], MESH, "uv_layers"),
];

impl Known {
    /// Whether this entry explains `m`; `at` is the statement of a `lines` mismatch.
    pub fn matches(&self, statements: &[String], at: Option<&str>, m: &Mismatch) -> bool {
        if self.comparator != "*" && self.comparator != m.comparator {
            return false;
        }
        if !self.needle.is_empty()
            && !m.path.contains(self.needle)
            && !m.detail.contains(self.needle)
        {
            return false;
        }
        let anywhere = |p: &str| statements.iter().any(|s| s.contains(p));
        match (m.comparator, at, self.statements.split_first()) {
            ("lines", Some(at), Some((first, rest))) => {
                at.contains(first) && rest.iter().all(|p| anywhere(p))
            }
            ("lines", None, _) => false,
            _ => self.statements.iter().all(|p| anywhere(p)),
        }
    }
}

/// The entry explaining `m`, if any; `at` is the statement of a `lines` mismatch.
pub fn explain(statements: &[String], at: Option<&str>, m: &Mismatch) -> Option<&'static Known> {
    KNOWN.iter().find(|k| k.matches(statements, at, m))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_name_a_deviation_row() {
        let text = include_str!("../../../../docs/deviations.md");
        for k in KNOWN {
            assert!(!k.statements.is_empty(), "{k:?}");
            if let Some(id) = k.deviation.strip_prefix("D-") {
                assert!(
                    text.contains(&format!("| D-{id} |")),
                    "{} is not a row of docs/deviations.md",
                    k.deviation
                );
            } else {
                let words = k.deviation.strip_prefix("§2 ").expect("`D-nn` or `§2 …`");
                let section2 = &text[text.find("## 2.").expect("§2")..];
                assert!(section2.contains(words), "no §2 row mentions `{words}`");
            }
        }
    }

    #[test]
    fn matching() {
        let k = diff("D-01", &["mesh.rip", "mode_set"], "lines", "TypeError");
        let m = Mismatch::new("lines", "line 5", "frap TypeError: x != blender ok");
        let s = vec![
            "bpy.ops.object.mode_set(mode='EDIT')".to_owned(),
            "bpy.ops.mesh.rip()".to_owned(),
        ];
        assert!(k.matches(&s, Some(&s[1]), &m));
        // The first pattern must be the failing statement.
        assert!(!k.matches(&s, Some(&s[0]), &m));
        assert!(!k.matches(&s[1..], Some(&s[1]), &m));
        assert!(!k.matches(
            &s,
            Some(&s[1]),
            &Mismatch::new("rna_values", "x", "TypeError")
        ));
        let k = diff("D-01", &["mesh.rip"], "mesh_canonical", "");
        assert!(k.matches(
            &s,
            None,
            &Mismatch::new("mesh_canonical", "meshes['Cube']", "")
        ));
    }
}
