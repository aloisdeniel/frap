// SPDX-License-Identifier: GPL-3.0-or-later
//! Explained mismatches: differences the differential fuzzer finds that `docs/deviations.md`
//! records (a deliberate deviation of §1 or a temporary gap of §2), so a run reports them as
//! explained instead of unexplained (WP 8.7).
//!
//! An entry explains a mismatch by its **cause**, never by co-occurrence: a statement of the
//! sequence matching [`Known::cause`] must have run the documented way, and the mismatch must
//! be the documented difference.
//!
//! - A `lines` mismatch (the statement's outcome differs): the mismatching statement itself
//!   matches `cause`, its outcome text contains `detail`, and every [`Known::with`] pattern
//!   matches a statement *before* it that succeeded in both tools.
//! - A state mismatch (`mesh_canonical`, `rna_values`: the end state differs): its field (the
//!   path after `meshes["Cube"].`, `evaluated["Cube"].`, `objects["Cube"].` or `context.`)
//!   is one of [`Known::fields`]; a statement matching `cause` succeeded in both tools (or
//!   failed in both with [`Known::fails_with`]) and acted on the mismatched mesh or object
//!   (it was active, selected or in Edit Mode after the statement, [`Trace`]); so did a
//!   statement for every `with` pattern. A `max_delta` entry only explains numbers that
//!   differ by at most that much; an `edit_end` entry only an object that ends in Edit Mode.
//! - A state mismatch explained this way also explains a `lines` mismatch of a later
//!   statement that acted on the same mesh or object (it ran on other geometry).
//! - A `cascade` entry (a statement that leaves the scene different from there on: an
//!   unimplemented case, a differing poll) also explains the mismatches *after* its cause:
//!   `lines` mismatches of later statements and state mismatches of meshes and objects a
//!   statement at or after the cause acted on (and of `context`).
//!
//! A pattern is a statement substring; `a|b` matches either, `a&b` needs both in the one
//! statement. A `with` or state `cause` pattern starting with `~` also accepts a statement
//! that failed (or was CANCELLED) the same way in both tools: some operators change state
//! before they cancel.
//!
//! Entries are found by fuzzing and triaged by hand: a bug is fixed by the owner of the code
//! (its minimised case then joins `tests/golden/cases/fuzz/`); a documented difference gets
//! an entry here naming its `docs/deviations.md` row (checked by a test).

use std::collections::{BTreeMap, BTreeSet};

use crate::case::Mismatch;

/// How one statement ran in the two tools.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// It ran without an error (and was not CANCELLED) in both tools.
    Ok,
    /// It failed the same way in both tools; frap's message.
    Failed(String),
    /// The outcomes differ (a `lines` mismatch).
    Differs,
}

/// What each statement of a run did: outcomes from both tools' line results, the meshes and
/// objects it acted on from a frap run (names at the end of the sequence).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Trace {
    /// Per statement.
    pub outcomes: Vec<Outcome>,
    /// Per statement: the mesh data of the objects that were active, selected or in Edit
    /// Mode after it, plus meshes it names (`bpy.data.meshes['…']`).
    pub meshes: Vec<BTreeSet<String>>,
    /// Per statement: the objects that were active, selected or in Edit Mode after it, plus
    /// objects it names.
    pub objects: Vec<BTreeSet<String>>,
    /// Objects in Edit Mode at the end.
    pub edit_end: BTreeSet<String>,
    /// The mesh of each mesh object at the end (an evaluated object is acted on through its
    /// mesh too: a linked duplicate edited, a tool on another user of the mesh).
    pub object_mesh: BTreeMap<String, String>,
}

impl Trace {
    fn ok(&self, i: usize) -> bool {
        self.outcomes.get(i) == Some(&Outcome::Ok)
    }

    /// Whether statement `i` ran as a [`Known::with`] pattern needs: succeeded in both tools,
    /// or for a `~` pattern ran the same way in both (it may have failed or been CANCELLED:
    /// some operators change state before they cancel).
    fn ran(&self, i: usize, pattern: &str) -> bool {
        if pattern.starts_with('~') {
            matches!(self.outcomes.get(i), Some(Outcome::Ok | Outcome::Failed(_)))
        } else {
            self.ok(i)
        }
    }

    fn touches(&self, i: usize, t: &Target) -> bool {
        match t {
            Target::Mesh(n) => self.meshes.get(i).is_some_and(|s| s.contains(n)),
            Target::Object(n) => {
                self.objects.get(i).is_some_and(|s| s.contains(n))
                    || self
                        .object_mesh
                        .get(n)
                        .is_some_and(|m| self.meshes.get(i).is_some_and(|s| s.contains(m)))
            }
            Target::Context => true,
        }
    }

    /// Whether a statement at or after `from` acted on `t`, or `t` is something frap never
    /// had (an object only Blender made after a cascade cause).
    fn touched_from(&self, from: usize, t: &Target) -> bool {
        let n = self.meshes.len();
        (from..n).any(|i| self.touches(i, t)) || !(0..n).any(|i| self.touches(i, t))
    }
}

/// What a state mismatch is about.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Target {
    Mesh(String),
    Object(String),
    Context,
}

/// The target and the field (`positions[3]`, `attributes["crease_edge"]`, `""` for the
/// whole mesh) of a state mismatch path.
fn target(path: &str) -> Option<(Target, &str)> {
    if let Some(f) = path.strip_prefix("context") {
        return Some((Target::Context, f.trim_start_matches('.')));
    }
    for (prefix, mesh) in [
        ("meshes[", true),
        ("evaluated[", false),
        ("objects[", false),
    ] {
        let Some(rest) = path.strip_prefix(prefix) else {
            continue;
        };
        let q = rest.chars().next()?;
        let rest = &rest[q.len_utf8()..];
        let end = rest.find(q)?;
        let name = rest[..end].to_owned();
        let field = rest[end + q.len_utf8()..]
            .trim_start_matches(']')
            .trim_start_matches('.');
        let t = if mesh {
            Target::Mesh(name)
        } else {
            Target::Object(name)
        };
        return Some((t, field));
    }
    None
}

/// Whether `pattern` (`a|b`, `a&b`, a leading `~` ignored) matches `statement`.
fn pattern_matches(pattern: &str, statement: &str) -> bool {
    pattern
        .trim_start_matches('~')
        .split('|')
        .any(|alt| alt.split('&').all(|p| statement.contains(p)))
}

/// The largest difference between the two numbers or number lists of a `frap [..] !=
/// blender [..]` detail (`frap 6008 != blender 6009 (tol …)`), if it has that form.
fn delta(detail: &str) -> Option<f64> {
    let nums = |s: &str| -> Option<Vec<f64>> {
        let s = s.trim();
        match s.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            Some(list) => list.split(',').map(|x| x.trim().parse().ok()).collect(),
            None => Some(vec![s.parse().ok()?]),
        }
    };
    let detail = detail.split(" (tol ").next().unwrap_or(detail);
    let (f, b) = detail.split_once(" != ")?;
    let f = nums(f.trim_start().strip_prefix("frap")?)?;
    let b = nums(b.trim_start().strip_prefix("blender")?)?;
    (f.len() == b.len()).then(|| {
        f.iter()
            .zip(&b)
            .map(|(x, y)| (x - y).abs())
            .fold(0.0, f64::max)
    })
}

/// One explained mismatch (see the module docs).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Known {
    /// The `docs/deviations.md` row: `D-nn`, or `§2 <words of the row>`.
    pub deviation: &'static str,
    /// The comparator (`lines`, `rna_values`, `mesh_canonical`).
    pub comparator: &'static str,
    /// The statement that causes the mismatch.
    pub cause: &'static str,
    /// Statements that must have succeeded before (`lines`) or on the same mesh or object.
    pub with: &'static [&'static str],
    /// `lines`: a substring of the outcome text (`frap ok != blender CANCELLED`); `""` any.
    pub detail: &'static str,
    /// State mismatches: field prefixes (`positions`, `edges`, `""` for counts); empty: any.
    pub fields: &'static [&'static str],
    /// State mismatches: the cause fails in both tools with this message (`""`: succeeds).
    pub fails_with: &'static str,
    /// State mismatches: the numbers differ by at most this much (`0.0`: any).
    pub max_delta: f64,
    /// State mismatches: the object ends in Edit Mode.
    pub edit_end: bool,
    /// Explains the mismatches after the cause too.
    pub cascade: bool,
}

const LINES: &str = "lines";
const MESH: &str = "mesh_canonical";
const RNA: &str = "rna_values";

const BASE: Known = Known {
    deviation: "",
    comparator: LINES,
    cause: "",
    with: &[],
    detail: "",
    fields: &[],
    fails_with: "",
    max_delta: 0.0,
    edit_end: false,
    cascade: false,
};

/// A `lines` difference at `cause` whose outcome text contains `detail`.
const fn line(deviation: &'static str, cause: &'static str, detail: &'static str) -> Known {
    Known {
        deviation,
        cause,
        detail,
        ..BASE
    }
}

/// A `lines` difference at `cause` that leaves the scene different from there on.
const fn gap(deviation: &'static str, cause: &'static str, detail: &'static str) -> Known {
    Known {
        cascade: true,
        ..line(deviation, cause, detail)
    }
}

/// A state difference of `fields` (empty: any) on what `cause` acted on.
const fn state(
    deviation: &'static str,
    comparator: &'static str,
    cause: &'static str,
    fields: &'static [&'static str],
) -> Known {
    Known {
        deviation,
        comparator,
        cause,
        fields,
        ..BASE
    }
}

const fn mesh(
    deviation: &'static str,
    cause: &'static str,
    fields: &'static [&'static str],
) -> Known {
    state(deviation, MESH, cause, fields)
}

impl Known {
    const fn with(self, with: &'static [&'static str]) -> Known {
        Known { with, ..self }
    }
    const fn cascade(self) -> Known {
        Known {
            cascade: true,
            ..self
        }
    }
    const fn edit_end(self) -> Known {
        Known {
            edit_end: true,
            ..self
        }
    }
    const fn max_delta(self, max_delta: f64) -> Known {
        Known { max_delta, ..self }
    }
    const fn fails_with(self, fails_with: &'static str) -> Known {
        Known { fails_with, ..self }
    }
}

const D_74: &str = "D-74";
const D_111: &str = "D-111";
const POLL: &str = "poll() failed";
const INCORRECT: &str = "context is incorrect";
const TORUS: &str = "§2 `mesh.primitive_torus_add` while a mesh is in Edit Mode";
const QSORT: &str = "§2 Ties in the C library `qsort` outside `uv.smart_project`";
const SNAP: &str =
    "§2 Differential fuzzing (WP 8.7), unimplemented modes: `view3d.snap_cursor_to_selected`";
const TRANSFORM_OBJECT: &str =
    "§2 Differential fuzzing (WP 8.7), unimplemented modes: `transform.mirror`";
const POLLS: &str = "§2 Differential fuzzing (WP 8.7), operator polls and outcomes";
const GEOMETRY: &str = "§2 Differential fuzzing (WP 8.7), geometry differences";
const UV: &str = "§2 Differential fuzzing (WP 8.7), UV results";
const FOUND: &str = "§2 Differential fuzzing (WP 8.7), causal triage";

/// Field groups.
const ANY: &[&str] = &[];
const POSITIONS: &[&str] = &["positions", "corner_normals", "face_normals"];
const NORMALS: &[&str] = &[
    "corner_normals",
    "face_normals",
    "attributes[\"custom_normal\"]",
];
const UVS: &[&str] = &["uv_layers"];
/// Faces wound the other way (and what face normals decide: sharp edges by angle).
const WINDING: &[&str] = &[
    "faces",
    "corner_normals",
    "face_normals",
    "",
    "attributes[\"sharp_edge\"]",
];
const OBJECT_TRANSFORM: &[&str] = &["location", "rotation", "scale", "matrix", "delta_"];

/// Edge data (and the corner normals sharp edges change).
const EDIT_EDGE_DATA: &[&str] = &[
    "attributes[\"crease_edge\"]",
    "attributes[\"sharp_edge\"]",
    "attributes[\"uv_seam\"]",
    "attributes[\"bevel_weight_edge\"]",
    "corner_normals",
];
/// The custom-normal tools.
const CUSTOM_NORMALS: &str = "mesh.smooth_normals|mesh.average_normals|mesh.merge_normals\
     |mesh.split_normals|mesh.set_normals_from_faces|mesh.normals_tools|mesh.point_normals\
     |mesh.customdata_custom_splitnormals_add";

/// Operators that change the topology (and so interpolate corner data).
const TOPOLOGY: &str = "mesh.dissolve_|mesh.unsubdivide|mesh.merge(|mesh.remove_doubles\
     |mesh.edge_split|mesh.delete|mesh.quads_convert_to_tris|mesh.tris_convert_to_quads\
     |mesh.subdivide|mesh.beautify_fill|mesh.edge_rotate|mesh.decimate|mesh.inset\
     |mesh.vert_connect|mesh.split(|mesh.extrude_|mesh.bevel|mesh.poke|mesh.bridge_edge_loops\
     |mesh.fill|mesh.spin|mesh.solidify|mesh.wireframe|mesh.duplicate|mesh.offset_edge_loops\
     |mesh.edge_face_add|mesh.triangulate|mesh.rip|mesh.bisect|mesh.knife";

/// The explained mismatches (see the module docs).
pub const KNOWN: &[Known] = &[
    // The startup Cube (and new meshes) store no active attribute.
    gap(D_74, "geometry.attribute_remove", ""),
    gap(D_74, "geometry.attribute_convert", ""),
    gap(TORUS, "mesh.primitive_torus_add", "not implemented"),
    // UV-editor-only selection operators run as in an Image Editor (headless-extended): frap
    // changes the UV selection where Blender's poll fails.
    gap(D_111, "uv.select_more", POLL),
    gap(D_111, "uv.select_less", POLL),
    gap(D_111, "uv.select_mode", POLL),
    gap(D_111, "uv.select_similar", POLL),
    gap(D_111, "uv.shortest_path_select", POLL),
    // The box packer's qsort ties.
    mesh(QSORT, "uv.lightmap_pack", UVS),
    gap(SNAP, "bpy.ops.view3d.snap_", "is not implemented yet"),
    gap(
        TRANSFORM_OBJECT,
        "bpy.ops.transform.mirror|bpy.ops.transform.tosphere|bpy.ops.transform.push_pull",
        "in OBJECT mode yet",
    ),
    // --- operator polls and outcomes ---
    gap(POLLS, "object.modifiers_clear", INCORRECT),
    line(POLLS, "object.parent_no_inverse_set", INCORRECT),
    // Edit Mode on an object hidden with `hide_view_set`.
    gap(
        POLLS,
        "object.material_slot_select|object.material_slot_deselect",
        "frap ok != blender CANCELLED",
    )
    .with(&["hide_view_set", "mode_set(mode='EDIT')"]),
    gap(
        POLLS,
        "mesh.dissolve_mode|mesh.select_mode",
        "frap CANCELLED != blender ok",
    )
    .with(&["hide_view_set", "mode_set(mode='EDIT')"]),
    gap(POLLS, "object.vertex_group_normalize_all", "locked"),
    // `uv.hide` with sync selection leaves another mesh selection.
    mesh(POLLS, "uv.hide", &["selection", "active", "select_history"]),
    gap(POLLS, "bpy.ops.transform.", "CANCELLED").with(&["uv.hide"]),
    gap(POLLS, "object.modifier_apply", "disabled"),
    gap(POLLS, "object.vertex_group_copy", "no vertex groups").with(&["object.modifier_apply"]),
    line(POLLS, "object.vertex_group_normalize(", "CANCELLED"),
    line(POLLS, "object.align", "UnboundLocalError"),
    state(POLLS, RNA, "uv.lightmap_pack", &["mode", "select_mode"])
        .fails_with("No mesh object")
        .cascade(),
    gap(POLLS, "bpy.ops.transform.", "locked").with(&["shape_key_lock"]),
    line(POLLS, "object.transform_apply", "aborting"),
    // --- geometry differences ---
    mesh(GEOMETRY, "mesh.loop_to_region", ANY),
    mesh(GEOMETRY, "mesh.flatten", ANY),
    mesh(GEOMETRY, "mesh.vert_connect_nonplanar", ANY),
    mesh(GEOMETRY, "mesh.extrude_edges_", WINDING).with(&["mode_set(mode='EDIT')"]),
    mesh(GEOMETRY, "mesh.extrude_context", ANY).with(&["mesh.primitive_uv_sphere_add"]),
    state(
        GEOMETRY,
        RNA,
        "ORIGIN_CENTER_OF_VOLUME",
        &["location", "matrix"],
    ),
    mesh(GEOMETRY, "ORIGIN_CENTER_OF_VOLUME", POSITIONS),
    // The evaluated SUBSURF result lacks the edge data the tools write; in Edit Mode
    // (`edit_end`) it also has other edges after some extrusions.
    mesh(
        GEOMETRY,
        "transform.edge_crease|shade_smooth_by_angle|mesh.mark_sharp|mesh.split_normals\
         |mark_seam|transform.edge_bevelweight|mesh.set_sharpness_by_angle",
        EDIT_EDGE_DATA,
    )
    .with(&["SUBSURF|subdivision_set"]),
    mesh(
        GEOMETRY,
        "mesh.extrude_edges_indiv",
        &["edges", "faces", ""],
    )
    .with(&["SUBSURF|subdivision_set"])
    .edit_end(),
    mesh(GEOMETRY, "extrude_region&use_normal_flip=True", ANY)
        .with(&["SUBSURF|subdivision_set"])
        .edit_end(),
    mesh(GEOMETRY, "object.transform_apply", POSITIONS)
        .with(&["shape_key_add", "SUBSURF|subdivision_set"]),
    // A few units of the custom normals' 16-bit encoding.
    mesh(GEOMETRY, CUSTOM_NORMALS, &["attributes[\"custom_normal\"]"]).max_delta(4.0),
    mesh(GEOMETRY, "mesh.dissolve_mode&use_face_split=True", NORMALS).with(&["mesh.normals_tools"]),
    mesh(GEOMETRY, "mesh.edge_face_add", ANY).with(&["mode_set(mode='EDIT')"]),
    mesh(GEOMETRY, "mesh.decimate", ANY),
    mesh(GEOMETRY, "uv.align", UVS),
    mesh(GEOMETRY, "mesh.fill(", ANY),
    mesh(GEOMETRY, "BEVEL", UVS),
    mesh(GEOMETRY, "mesh.edge_split", NORMALS).with(&["mesh.merge_normals"]),
    mesh(GEOMETRY, "mesh.edge_rotate", ANY).with(&["mode_set(mode='EDIT')"]),
    mesh(GEOMETRY, "mesh.normals_make_consistent", ANY)
        .with(&["mode_set(mode='EDIT')", "mesh.primitive_plane_add"]),
    mesh(GEOMETRY, "mesh.primitive_", POSITIONS).with(&["shape_key_add", "mode_set(mode='EDIT')"]),
    mesh(GEOMETRY, "uv.weld", UVS).with(&["mesh.split"]),
    mesh(GEOMETRY, "mesh.convex_hull", ANY),
    mesh(
        GEOMETRY,
        "mesh.fill_grid",
        &["attributes[\"material_index\"]"],
    )
    .with(&["material_slot_add"]),
    mesh(GEOMETRY, "mesh.quads_convert_to_tris", ANY),
    mesh(GEOMETRY, "object.transform_apply", POSITIONS).with(&["shape_key_add"]),
    state(
        GEOMETRY,
        RNA,
        "view3d.snap_selected_to_cursor",
        OBJECT_TRANSFORM,
    )
    .with(&["object.parent_set&keep_transform=True"]),
    // Corner normals within about 1e-3.
    mesh(GEOMETRY, "use_proportional_edit=True", NORMALS).max_delta(2e-3),
    mesh(GEOMETRY, "mesh.spin&use_normal_flip=True", NORMALS).max_delta(2e-3),
    mesh(
        GEOMETRY,
        "mesh.smooth_normals|mesh.average_normals|mesh.merge_normals|mesh.split_normals\
         |mesh.set_normals_from_faces|mesh.customdata_custom_splitnormals_add",
        NORMALS,
    )
    .max_delta(2e-3),
    mesh(
        GEOMETRY,
        "mesh.solidify|mesh.extrude_vertices_move|object.origin_set",
        POSITIONS,
    )
    .with(&["shape_key_add", "mode_set(mode='EDIT')"]),
    mesh(GEOMETRY, "transform.mirror", ANY).with(&["delta_", "mode_set(mode='EDIT')"]),
    mesh(GEOMETRY, "mesh.bisect&use_fill=True", ANY),
    mesh(GEOMETRY, "mesh.dissolve_limited|mesh.delete_edgeloop", ANY)
        .with(&["mesh.edge_split", "mesh.primitive_"]),
    mesh(GEOMETRY, "transform.vertex_random", POSITIONS),
    mesh(GEOMETRY, "transform.shrink_fatten", POSITIONS).with(&["mesh.extrude_faces_indiv"]),
    mesh(GEOMETRY, "orient_type='NORMAL'", POSITIONS).with(&["mode_set(mode='EDIT')"]),
    // Blender's own UV sphere element order changes between runs (D-19): results that
    // depend on the element order (index writes, ties) are not comparable on its mesh.
    mesh("D-19", "mesh.primitive_uv_sphere_add", ANY),
    // --- found by the causal triage (WP 8.7 fix round) ---
    mesh(FOUND, "transform.shear", NORMALS).with(&["mesh.subdivide", "mesh.faces_shade_smooth"]),
    mesh(FOUND, "mesh.bevel", ANY).with(&["mesh.extrude_faces_indiv"]),
    mesh(FOUND, "mesh.bridge_edge_loops", ANY),
    mesh(FOUND, "mesh.extrude_region", NORMALS).with(&["mesh.dissolve_edges"]),
    mesh(FOUND, "transform.tosphere", NORMALS).with(&["mesh.extrude_faces_indiv"]),
    mesh(FOUND, "transform.shear", ANY)
        .with(&["mode_set(mode='EDIT')"])
        .edit_end(),
    mesh(FOUND, "mesh.dissolve_edges", NORMALS)
        .with(&["mesh.set_sharpness_by_angle|mesh.mark_sharp"]),
    state(FOUND, RNA, "object.align", OBJECT_TRANSFORM).with(&["SUBSURF|subdivision_set"]),
    mesh(FOUND, TOPOLOGY, NORMALS).with(&[CUSTOM_NORMALS]),
    mesh(FOUND, "mesh.solidify", POSITIONS).with(&["mesh.dissolve_edges"]),
    mesh(
        FOUND,
        "transform.shrink_fatten&use_even_offset=True",
        POSITIONS,
    ),
    mesh(FOUND, "transform.shear&angle=0.0,", ANY),
    mesh(FOUND, "mesh.tris_convert_to_quads", ANY).with(&["TRIFAN"]),
    state(FOUND, RNA, "bpy.ops.transform.rotate", &["rotation"])
        .with(&["object.transforms_to_deltas"]),
    mesh(FOUND, "use_boundary_tear=True", ANY),
    mesh(FOUND, "transform.push_pull", POSITIONS).with(&["mode_set(mode='EDIT')"]),
    // A UV selection operator after `uv.select_split` (which may leave Blender's UV
    // selection partly independent of the mesh selection even when it cancels).
    mesh(D_111, "bpy.ops.uv.select", ANY).with(&["~uv.select_split"]),
    mesh(FOUND, "BEVEL", &["corner_normals", "face_normals"]).max_delta(1e-3),
    mesh(FOUND, "mesh.bevel&affect='VERTICES'", UVS),
    mesh(
        FOUND,
        "mesh.bevel",
        &["corner_normals", "face_normals", "uv_layers"],
    )
    .max_delta(2e-3),
    mesh(FOUND, "TRIANGULATE", ANY).with(&["mesh.subdivide"]),
    mesh(FOUND, "mesh.primitive_cube_add", UVS).with(&[
        "transform.rotate",
        "mode_set(mode='EDIT')",
        "mesh.tris_convert_to_quads",
    ]),
    mesh(FOUND, "mesh.primitive_grid_add", UVS).with(&[
        "mode_set(mode='EDIT')",
        "mesh.extrude_repeat",
        "mesh.bisect",
    ]),
    mesh(FOUND, "mesh.separate", &["attributes[\"__mirror_index\"]"]).with(&["mesh.select_mirror"]),
    mesh(FOUND, "mesh.remove_doubles", ANY),
    mesh(FOUND, "~mesh.edge_face_add", &["selection"]).with(&["mesh.region_to_loop"]),
    mesh(FOUND, "mesh.select_random", ANY).with(&[TOPOLOGY]),
    mesh(FOUND, "mesh.vert_connect", NORMALS).with(&["mesh.sort_elements"]),
    mesh(FOUND, "mesh.duplicate", ANY).with(&["mesh.wireframe"]),
    mesh(FOUND, "WELD", WINDING).with(&["mesh.edge_split"]),
    // --- UV results after Edit Mode geometry changes ---
    mesh(UV, "uv.unwrap", UVS),
    mesh(UV, "uv.smart_project", UVS),
    mesh(UV, "uv.sphere_project", UVS),
    mesh(UV, "uv.cube_project", UVS),
    mesh(UV, "uv.cylinder_project", UVS),
    mesh(UV, "uv.remove_doubles", UVS),
    mesh(UV, "uv.weld", UVS),
    mesh(UV, "uv.average_islands_scale", UVS),
];

impl Known {
    /// Whether this entry explains `m` directly; on success the index of the cause.
    pub fn explains(&self, statements: &[String], trace: &Trace, m: &Mismatch) -> Option<usize> {
        if self.comparator != m.comparator {
            return None;
        }
        if m.comparator == LINES {
            let i = line_index(&m.path)?;
            let ok = statements
                .get(i)
                .is_some_and(|s| pattern_matches(self.cause, s))
                && m.detail.contains(self.detail)
                && self
                    .with
                    .iter()
                    .all(|p| (0..i).any(|j| trace.ran(j, p) && pattern_matches(p, &statements[j])));
            return ok.then_some(i);
        }
        let (t, field) = target(&m.path)?;
        // `""` is the whole mesh (counts, `… unmatched … in total`), a name a field prefix.
        let field_is = |f: &&str| {
            if f.is_empty() {
                field.is_empty()
            } else {
                field.starts_with(f)
            }
        };
        if !self.fields.is_empty() && !self.fields.iter().any(field_is) {
            return None;
        }
        // The custom normals' second component is an angle in `[-π, π]` coded on ±32767:
        // its two ends are the same direction.
        let periodic = |d: f64| {
            if field.starts_with("attributes[\"custom_normal\"]") {
                d.min((65534.0 - d).abs())
            } else {
                d
            }
        };
        if self.max_delta > 0.0
            && delta(&m.detail).is_none_or(|d| periodic(d) > self.max_delta)
        {
            return None;
        }
        if self.edit_end {
            let in_edit = match (&t, m.path.starts_with("evaluated[")) {
                (Target::Object(n), true) => trace.edit_end.contains(n),
                _ => false,
            };
            if !in_edit {
                return None;
            }
        }
        let ran = |i: usize| {
            if self.fails_with.is_empty() {
                trace.ran(i, self.cause)
            } else {
                matches!(trace.outcomes.get(i), Some(Outcome::Failed(f)) if f.contains(self.fails_with))
            }
        };
        let cause = (0..statements.len()).rev().find(|&i| {
            pattern_matches(self.cause, &statements[i]) && ran(i) && trace.touches(i, &t)
        })?;
        let with = self.with.iter().all(|p| {
            (0..statements.len()).any(|j| {
                trace.ran(j, p) && pattern_matches(p, &statements[j]) && trace.touches(j, &t)
            })
        });
        with.then_some(cause)
    }
}

/// The statement index of a `lines` mismatch path (`line 7`).
fn line_index(path: &str) -> Option<usize> {
    let n: usize = path.strip_prefix("line ")?.parse().ok()?;
    n.checked_sub(super::driver::PREAMBLE_LINES as usize + 1)
}

/// The entry explaining each mismatch, `None` where nothing does (see the module docs).
pub fn explain_all(
    statements: &[String],
    trace: &Trace,
    mismatches: &[Mismatch],
) -> Vec<Option<&'static Known>> {
    let mut direct: Vec<Option<(&'static Known, usize)>> = mismatches
        .iter()
        .map(|m| {
            KNOWN
                .iter()
                .find_map(|k| k.explains(statements, trace, m).map(|i| (k, i)))
        })
        .collect();
    // A comparator's `… and N more differing elements` continues its listed elements of the
    // same path (`corner_normals[3]`, …): it takes the entry that explains them all.
    for (i, m) in mismatches.iter().enumerate() {
        if direct[i].is_some() || !m.detail.starts_with("… and ") {
            continue;
        }
        let listed: Vec<usize> = (0..mismatches.len())
            .filter(|&j| {
                j != i
                    && mismatches[j].comparator == m.comparator
                    && mismatches[j]
                        .path
                        .strip_prefix(m.path.as_str())
                        .is_some_and(|r| r.starts_with('['))
            })
            .collect();
        if !listed.is_empty() && listed.iter().all(|&j| direct[j].is_some()) {
            direct[i] = direct[listed[0]];
        }
    }
    // A cascade entry whose cause fails in both tools with its documented message is
    // triggered by that statement alone (what follows may show no state mismatch of its own:
    // `uv.lightmap_pack` failing in Edit Mode, then a mode-dependent poll).
    let failed_causes = KNOWN
        .iter()
        .filter(|k| k.cascade && !k.fails_with.is_empty())
        .flat_map(|k| {
            (0..statements.len()).filter_map(move |i| {
                let failed = matches!(trace.outcomes.get(i),
                    Some(Outcome::Failed(f)) if f.contains(k.fails_with));
                (failed && pattern_matches(k.cause, &statements[i])).then_some((k, i))
            })
        });
    // The earliest cascade cause explains what follows it.
    let cascade = direct
        .iter()
        .flatten()
        .copied()
        .filter(|(k, _)| k.cascade)
        .chain(failed_causes)
        .min_by_key(|(_, i)| *i);
    // A documented end-state difference of a mesh or object (its cause found) also explains
    // a later statement that acts on that mesh or object and so runs differently.
    let state_causes: Vec<(&'static Known, usize, Target)> = mismatches
        .iter()
        .zip(&direct)
        .filter(|(m, _)| m.comparator != LINES)
        .filter_map(|(m, d)| {
            let (k, i) = (*d)?;
            let (t, _) = target(&m.path)?;
            (t != Target::Context).then_some((k, i, t))
        })
        .collect();
    mismatches
        .iter()
        .zip(&direct)
        .map(|(m, d)| {
            if let Some((k, _)) = d {
                return Some(*k);
            }
            if m.comparator == LINES
                && let Some(j) = line_index(&m.path)
                && let Some((k, _, _)) = state_causes
                    .iter()
                    .find(|(_, i, t)| *i < j && trace.touches(j, t))
            {
                return Some(*k);
            }
            let (k, from) = cascade?;
            let after = if m.comparator == LINES {
                line_index(&m.path).is_some_and(|i| i > from)
            } else {
                target(&m.path).is_some_and(|(t, _)| trace.touched_from(from, &t))
            };
            after.then_some(k)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_name_a_deviation_row() {
        let text = include_str!("../../../../docs/deviations.md");
        for k in KNOWN {
            assert!(!k.cause.is_empty(), "{k:?}");
            assert!(matches!(k.comparator, LINES | MESH | RNA), "{k:?}");
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

    fn st(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| (*s).to_owned()).collect()
    }

    /// A trace where every statement succeeded and acted on the Cube.
    fn cube_trace(n: usize) -> Trace {
        let one = |s: &str| BTreeSet::from([s.to_owned()]);
        Trace {
            outcomes: vec![Outcome::Ok; n],
            meshes: vec![one("Cube"); n],
            objects: vec![one("Cube"); n],
            edit_end: BTreeSet::new(),
            object_mesh: BTreeMap::new(),
        }
    }

    fn line_of(i: usize) -> String {
        format!(
            "line {}",
            super::super::driver::PREAMBLE_LINES as usize + 1 + i
        )
    }

    #[test]
    fn patterns_and_paths() {
        assert!(pattern_matches("a|b&c", "xbcx"));
        assert!(!pattern_matches("a|b&c", "xbx"));
        assert_eq!(
            target("meshes[\"Cube\"].positions[3]"),
            Some((Target::Mesh("Cube".into()), "positions[3]"))
        );
        assert_eq!(
            target("evaluated[\"Cube\"].attributes[\"crease_edge\"]"),
            Some((Target::Object("Cube".into()), "attributes[\"crease_edge\"]"))
        );
        assert_eq!(
            target("meshes[\"Torus\"]"),
            Some((Target::Mesh("Torus".into()), ""))
        );
        assert_eq!(target("context.mode"), Some((Target::Context, "mode")));
        assert_eq!(
            delta("frap [0.0, 0.5, 1.0] != blender [0.0, 0.5, 0.999]"),
            Some(0.0010000000000000009)
        );
        assert_eq!(delta("frap (1, 2) != blender (2, 3)"), None);
        assert_eq!(
            delta("frap 6008 != blender 6009 (tol 0.00009999999747378752)"),
            Some(1.0)
        );
        // The custom normals' angle wraps around.
        let k = mesh(GEOMETRY, CUSTOM_NORMALS, &["attributes[\"custom_normal\"]"]).max_delta(4.0);
        let s = vec!["bpy.ops.mesh.average_normals()".to_owned()];
        let m = |d: &str| {
            Mismatch::new(
                "mesh_canonical",
                "meshes[\"Cube\"].attributes[\"custom_normal\"].data[5][1]",
                d,
            )
        };
        let tr = cube_trace(1);
        assert!(k.explains(&s, &tr, &m("frap 32767 != blender -32767")).is_some());
        assert!(k.explains(&s, &tr, &m("frap 100 != blender -100")).is_none());
    }

    #[test]
    fn lines_entries_need_the_cause_and_earlier_successes() {
        let k = line("D-01", "mesh.rip", "TypeError").with(&["mode_set"]);
        let s = st(&["bpy.ops.object.mode_set(mode='EDIT')", "bpy.ops.mesh.rip()"]);
        let mut tr = cube_trace(2);
        let m = Mismatch::new("lines", line_of(1), "frap TypeError: x != blender ok");
        assert_eq!(k.explains(&s, &tr, &m), Some(1));
        // The cause must be the failing statement.
        let m0 = Mismatch::new("lines", line_of(0), "frap TypeError: x != blender ok");
        assert_eq!(k.explains(&s, &tr, &m0), None);
        // The context must have succeeded, before the cause.
        tr.outcomes[0] = Outcome::Failed("poll".into());
        assert_eq!(k.explains(&s, &tr, &m), None);
        let s2 = st(&["bpy.ops.mesh.rip()", "bpy.ops.object.mode_set(mode='EDIT')"]);
        let m2 = Mismatch::new("lines", line_of(0), "frap TypeError: x != blender ok");
        assert_eq!(k.explains(&s2, &cube_trace(2), &m2), None);
    }

    #[test]
    fn state_entries_need_a_successful_cause_on_the_mismatched_mesh() {
        let k = mesh(GEOMETRY, "mesh.decimate", ANY);
        let s = st(&[
            "bpy.ops.mesh.primitive_plane_add()",
            "bpy.ops.mesh.decimate(ratio=0.5)",
        ]);
        let m = Mismatch::new("mesh_canonical", "meshes[\"Cube\"].positions[1]", "x");
        let mut tr = cube_trace(2);
        assert_eq!(k.explains(&s, &tr, &m), Some(1));
        // A cause that failed (in both tools) did nothing.
        tr.outcomes[1] = Outcome::Failed("poll() failed".into());
        assert_eq!(k.explains(&s, &tr, &m), None);
        // A cause that acted on another mesh.
        let mut tr = cube_trace(2);
        tr.meshes[1] = BTreeSet::from(["Plane".to_owned()]);
        assert_eq!(k.explains(&s, &tr, &m), None);
        // Fields narrow it.
        let k = mesh(GEOMETRY, "mesh.decimate", UVS);
        assert_eq!(k.explains(&s, &cube_trace(2), &m), None);
        // So does the size of the difference.
        let k = mesh(GEOMETRY, "mesh.decimate", ANY).max_delta(1e-3);
        let small = Mismatch::new(
            "mesh_canonical",
            "meshes[\"Cube\"].corner_normals[1]",
            "frap [0.0, 0.0, 1.0] != blender [0.0, 0.0005, 1.0]",
        );
        let big = Mismatch::new(
            "mesh_canonical",
            "meshes[\"Cube\"].corner_normals[1]",
            "frap [0.0, 0.0, 1.0] != blender [0.0, 0.0, -1.0]",
        );
        assert!(k.explains(&s, &cube_trace(2), &small).is_some());
        assert!(k.explains(&s, &cube_trace(2), &big).is_none());
        // `""` is the whole mesh only.
        let k = mesh(GEOMETRY, "mesh.decimate", &[""]);
        let counts = Mismatch::new("mesh_canonical", "meshes[\"Cube\"]", "frap (8, 12, 6) != …");
        assert!(k.explains(&s, &cube_trace(2), &counts).is_some());
        assert!(k.explains(&s, &cube_trace(2), &m).is_none());
    }

    /// The verifier's case: a mismatch nothing explains (here a corner-normal flip after
    /// `extrude_region_move`, which no entry names) stays unexplained when an unrelated
    /// statement that fails the same way in both tools (and so does nothing) is appended, or
    /// one that succeeds on another mesh.
    #[test]
    fn a_known_mismatch_plus_an_unrelated_no_op_is_still_unexplained() {
        let s = st(&[
            "bpy.ops.object.mode_set(mode='EDIT')",
            "bpy.ops.transform.rotate(value=-1.86, orient_axis='Z')",
            "bpy.ops.mesh.extrude_region_move(TRANSFORM_OT_translate={'value': (0.5, -0.25, 0)})",
            "bpy.ops.object.mode_set(mode='OBJECT')",
            "bpy.ops.mesh.decimate(ratio=0.5)",
        ]);
        let m = Mismatch::new(
            "mesh_canonical",
            "meshes[\"Cube\"].corner_normals[12]",
            "frap [0.0, 0.0, 1.0] != blender [0.0, 0.0, -1.0]",
        );
        let mut tr = cube_trace(5);
        // The decimate as the cause: explained.
        assert!(explain_all(&s, &tr, std::slice::from_ref(&m))[0].is_some());
        // The decimate's poll fails in both tools: it did nothing.
        tr.outcomes[4] = Outcome::Failed(
            "Operator bpy.ops.mesh.decimate.poll() failed, context is incorrect".into(),
        );
        assert_eq!(explain_all(&s, &tr, std::slice::from_ref(&m)), vec![None]);
        // An appended statement that succeeds on another mesh does not explain it either.
        let mut s2 = s.clone();
        s2[4] = "bpy.ops.mesh.edge_face_add()".into();
        let mut tr = cube_trace(5);
        tr.meshes[4] = BTreeSet::from(["Plane".to_owned()]);
        tr.objects[4] = BTreeSet::from(["Plane".to_owned()]);
        assert_eq!(explain_all(&s2, &tr, std::slice::from_ref(&m)), vec![None]);
    }

    #[test]
    fn cascades_explain_only_what_follows_the_cause() {
        let s = st(&[
            "bpy.ops.object.mode_set(mode='EDIT')",
            "bpy.ops.mesh.extrude_region_move()",
            "bpy.ops.view3d.snap_cursor_to_selected()",
            "bpy.ops.mesh.primitive_cube_add()",
        ]);
        let mut tr = cube_trace(4);
        let gap = Mismatch::new(
            "lines",
            line_of(2),
            "frap NotImplementedError: frap: view3d.snap_cursor_to_selected in EDIT_MESH mode \
             is not implemented yet != blender ok",
        );
        let later = Mismatch::new("lines", line_of(3), "frap ok != blender CANCELLED");
        let earlier = Mismatch::new("lines", line_of(1), "frap ok != blender CANCELLED");
        let cube = Mismatch::new("mesh_canonical", "meshes[\"Cube\"].positions[0]", "x");
        let plane = Mismatch::new("mesh_canonical", "meshes[\"Plane\"].positions[0]", "x");
        tr.meshes[0].insert("Plane".into());
        // An object only Blender has (frap never made it).
        let only = Mismatch::new("rna_values", "objects[\"Cube.001\"]", "only in blender");
        let all = [gap, later, earlier, cube, plane, only];
        let e = explain_all(&s, &tr, &all);
        assert!(e[0].is_some() && e[1].is_some() && e[3].is_some(), "{e:?}");
        assert!(e[5].is_some(), "{e:?}");
        // Before the gap, or on a mesh nothing touched since.
        assert!(e[2].is_none() && e[4].is_none(), "{e:?}");
    }

    #[test]
    fn a_state_difference_explains_later_statements_on_its_mesh() {
        let s = st(&[
            "bpy.ops.object.mode_set(mode='EDIT')",
            "bpy.ops.mesh.dissolve_verts(use_boundary_tear=True)",
            "bpy.ops.mesh.face_make_planar()",
        ]);
        let state = Mismatch::new("mesh_canonical", "meshes[\"Cube\"]", "frap (8, 12, 6) != …");
        let later = Mismatch::new("lines", line_of(2), "frap CANCELLED != blender ok");
        let earlier = Mismatch::new("lines", line_of(0), "frap CANCELLED != blender ok");
        let mut tr = cube_trace(3);
        let all = [state.clone(), later.clone(), earlier];
        let e = explain_all(&s, &tr, &all);
        assert!(e[0].is_some() && e[1].is_some() && e[2].is_none(), "{e:?}");
        // Not when the later statement acted on another mesh.
        tr.meshes[2] = BTreeSet::from(["Plane".to_owned()]);
        tr.objects[2] = BTreeSet::from(["Plane".to_owned()]);
        let e = explain_all(&s, &tr, &[state, later]);
        assert!(e[0].is_some() && e[1].is_none(), "{e:?}");
    }

    #[test]
    fn a_continuation_takes_the_entry_of_its_listed_elements() {
        let s = st(&["bpy.ops.mesh.smooth_normals()"]);
        let k = |d: &str| Mismatch::new("mesh_canonical", "meshes[\"Cube\"].corner_normals[1]", d);
        let more = Mismatch::new(
            "mesh_canonical",
            "meshes[\"Cube\"].corner_normals",
            "… and 6 more differing elements",
        );
        let small = k("frap [0.0, 0.0, 1.0] != blender [0.0, 0.001, 1.0]");
        let e = explain_all(&s, &cube_trace(1), &[small, more.clone()]);
        assert!(e.iter().all(Option::is_some), "{e:?}");
        let big = k("frap [0.0, 0.0, 1.0] != blender [0.0, 0.0, -1.0]");
        let e = explain_all(&s, &cube_trace(1), &[big, more]);
        assert_eq!(e, vec![None, None]);
    }

    #[test]
    fn a_cascade_cause_failing_as_documented_triggers_alone() {
        let s = st(&[
            "bpy.ops.object.mode_set(mode='EDIT')",
            "bpy.ops.uv.lightmap_pack(PREF_PACK_IN_ONE=False)",
            "bpy.ops.mesh.fill()",
        ]);
        let mut tr = cube_trace(3);
        tr.outcomes[1] = Outcome::Failed("Error: No mesh object".into());
        let m = Mismatch::new(
            "lines",
            line_of(2),
            "frap RuntimeError: Error: No edges selected != blender RuntimeError: Operator \
             bpy.ops.mesh.fill.poll() failed, context is incorrect",
        );
        assert!(explain_all(&s, &tr, std::slice::from_ref(&m))[0].is_some());
        // Another failure of the same operator does not.
        tr.outcomes[1] = Outcome::Failed("Error: something else".into());
        assert_eq!(explain_all(&s, &tr, std::slice::from_ref(&m)), vec![None]);
    }

    #[test]
    fn evaluated_entries_need_edit_mode_at_the_end() {
        let s = st(&[
            "bpy.ops.object.modifier_add(type='SUBSURF')",
            "bpy.ops.object.mode_set(mode='EDIT')",
            "bpy.ops.mesh.extrude_region(use_normal_flip=True)",
        ]);
        let m = Mismatch::new(
            "mesh_canonical",
            "evaluated[\"Cube\"].edges[3]",
            "frap edge [1, 3] has no blender counterpart",
        );
        let mut tr = cube_trace(3);
        assert_eq!(explain_all(&s, &tr, std::slice::from_ref(&m)), vec![None]);
        tr.edit_end.insert("Cube".into());
        assert!(explain_all(&s, &tr, std::slice::from_ref(&m))[0].is_some());
        // The evaluated SUBSURF lacks the edge data whatever the mode.
        let s = st(&[
            "bpy.ops.object.modifier_add(type='SUBSURF')",
            "bpy.ops.object.mode_set(mode='EDIT')",
            "bpy.ops.transform.edge_crease(value=0.5)",
        ]);
        let c = Mismatch::new(
            "mesh_canonical",
            "evaluated[\"Cube\"].attributes[\"crease_edge\"]",
            "only in blender",
        );
        assert!(explain_all(&s, &cube_trace(3), std::slice::from_ref(&c))[0].is_some());
        // Other evaluated fields are not explained.
        let p = Mismatch::new("mesh_canonical", "evaluated[\"Cube\"].positions[0]", "x");
        assert_eq!(explain_all(&s, &tr, std::slice::from_ref(&p)), vec![None]);
    }
}
