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
//!   differ by at most that much; an `edit_end` entry only an object that ends in Edit Mode;
//!   an `anywhere` entry needs no statement to have acted on the object (Blender moved it
//!   through a statement frap ran another way).
//! - A state mismatch explained this way also explains a `lines` mismatch of a later
//!   statement that acted on the same mesh or object (it ran on other geometry).
//! - A `cascade` entry (a statement that leaves the scene different from there on: an
//!   unimplemented case, a differing poll) also explains the mismatches *after* its cause:
//!   `lines` mismatches of later statements and state mismatches of meshes and objects a
//!   statement at or after the cause acted on (and of `context`).
//!
//! A pattern is a statement substring; `a|b` matches either, `a&b` needs both in the one
//! statement, `a&!b` needs `a` without `b`. A `with` or state `cause` pattern starting with `~` also accepts a statement
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
    /// Per compared mesh of Blender's result (`meshes["Cube"]`, `evaluated["Cube"]`): the
    /// corners and faces whose normals float rounding decides ([`Degenerate::of`]).
    pub degenerate: BTreeMap<String, Degenerate>,
}

/// The corners and faces of a mesh (Blender's indices) whose normals float rounding decides:
/// the faces whose area is zero up to rounding (a wall `extrude_faces_indiv` or
/// `offset_edge_loops` leaves, a quad twisted onto its own plane: its Newell normal is the
/// sum of terms that cancel exactly; a face collapsed onto a point), their corners, and the
/// smooth corners around them (a vertex normal sums the faces' normals), and the smooth
/// corners of vertices whose face normals cancel up to rounding (overlapping faces of
/// opposite normals). §2 "Differential fuzzing (WP 8.7), causal triage".
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Degenerate {
    /// Face indices.
    pub faces: BTreeSet<usize>,
    /// Corner indices.
    pub corners: BTreeSet<usize>,
}

impl Degenerate {
    /// Twice the area below this fraction of the face's squared longest edge is zero.
    pub const REL_AREA: f64 = 1e-5;
    /// A face whose longest edge is below this fraction of its largest coordinate (or of 1,
    /// when larger) is collapsed onto a point (a corner BEVEL closes after another BEVEL):
    /// its normal is the rounding noise of coincident positions.
    pub const REL_EDGE: f64 = 1e-5;
    /// A smooth vertex normal whose angle-weighted sum of face normals is below this fraction
    /// of the angle sum cancels (its direction is rounding noise).
    pub const REL_CANCEL: f64 = 1e-5;

    /// From a dumped mesh.
    pub fn of(m: &crate::state::GMesh) -> Degenerate {
        let p = |i: u32| m.positions.get(i as usize).map(|c| c.map(f64::from));
        let mut d = Degenerate::default();
        let mut verts: BTreeSet<u32> = BTreeSet::new();
        // Per vertex: the angle-weighted sum of the unit face normals, and the sum of angles.
        let mut sums: BTreeMap<u32, ([f64; 3], f64)> = BTreeMap::new();
        for (fi, f) in m.faces.iter().enumerate() {
            let pts: Option<Vec<[f64; 3]>> = f.iter().map(|&v| p(v)).collect();
            let Some(pts) = pts else {
                continue;
            };
            let (mut n, mut longest, mut extent) = ([0.0f64; 3], 0.0f64, 1.0f64);
            for k in 0..pts.len() {
                let (a, b) = (pts[k], pts[(k + 1) % pts.len()]);
                n[0] += (a[1] - b[1]) * (a[2] + b[2]);
                n[1] += (a[2] - b[2]) * (a[0] + b[0]);
                n[2] += (a[0] - b[0]) * (a[1] + b[1]);
                let e = (0..3).map(|c| (a[c] - b[c]).powi(2)).sum::<f64>();
                longest = longest.max(e);
                extent = a.iter().fold(extent, |m, c| m.max(c.abs()));
            }
            let area2 = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            let collapsed = longest.sqrt() <= Self::REL_EDGE * extent;
            if collapsed || area2 <= Self::REL_AREA * longest {
                d.faces.insert(fi);
                verts.extend(f.iter().copied());
                continue;
            }
            let unit = n.map(|c| c / area2);
            for k in 0..pts.len() {
                let (prev, cur, next) = (
                    pts[(k + pts.len() - 1) % pts.len()],
                    pts[k],
                    pts[(k + 1) % pts.len()],
                );
                let dir = |o: [f64; 3]| {
                    let v = [o[0] - cur[0], o[1] - cur[1], o[2] - cur[2]];
                    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
                    if l > 0.0 { v.map(|c| c / l) } else { v }
                };
                let (a, b) = (dir(prev), dir(next));
                let angle = (a[0] * b[0] + a[1] * b[1] + a[2] * b[2])
                    .clamp(-1.0, 1.0)
                    .acos();
                let e = sums.entry(f[k]).or_insert(([0.0; 3], 0.0));
                for (acc, u) in e.0.iter_mut().zip(unit) {
                    *acc += u * angle;
                }
                e.1 += angle;
            }
        }
        // Vertices whose face normals cancel up to rounding (overlapping faces of opposite
        // normals: an in-plane extrusion, the coincident walls of individual extrusions): their
        // smooth normal is the direction of the rounding residue.
        for (v, (sum, weight)) in sums {
            let len = (sum[0] * sum[0] + sum[1] * sum[1] + sum[2] * sum[2]).sqrt();
            if weight > 0.0 && len <= Self::REL_CANCEL * weight {
                verts.insert(v);
            }
        }
        if verts.is_empty() {
            return d;
        }
        let sharp = m
            .attributes
            .get("sharp_face")
            .and_then(|a| a.get("data"))
            .and_then(|v| v.as_array());
        let mut corner = 0usize;
        for (fi, f) in m.faces.iter().enumerate() {
            let smooth = !sharp
                .and_then(|a| a.get(fi))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            for &v in f {
                if d.faces.contains(&fi) || (smooth && verts.contains(&v)) {
                    d.corners.insert(corner);
                }
                corner += 1;
            }
        }
        d
    }

    /// Whether `field` (`corner_normals[12]`, `face_normals[3]`) is one of these.
    fn covers(&self, field: &str) -> bool {
        let index = |prefix: &str| -> Option<usize> {
            field.strip_prefix(prefix)?.strip_suffix(']')?.parse().ok()
        };
        if let Some(c) = index("corner_normals[") {
            return self.corners.contains(&c);
        }
        if let Some(f) = index("face_normals[") {
            return self.faces.contains(&f);
        }
        false
    }
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

/// Whether `pattern` (`a|b`, `a&b`, `a&!b`: `a` without `b`, a leading `~` ignored) matches
/// `statement`.
fn pattern_matches(pattern: &str, statement: &str) -> bool {
    pattern.trim_start_matches('~').split('|').any(|alt| {
        alt.split('&').all(|p| match p.strip_prefix('!') {
            Some(not) => !statement.contains(not),
            None => statement.contains(p),
        })
    })
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
#[derive(Clone, Copy, Debug)]
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
    /// A further condition on the cause statement (its arguments), `None`: none.
    pub when: Option<fn(&str) -> bool>,
    /// State mismatches: only normals of [`Degenerate`] corners and faces of Blender's result.
    pub degenerate: bool,
    /// State mismatches of objects the statements need not have acted on (a statement of
    /// Blender's that frap did not run the same way moved them: proportional editing in
    /// Object Mode after a mode difference).
    pub anywhere: bool,
}

impl PartialEq for Known {
    /// Entries are distinct by their data; `when` only by presence (function addresses are
    /// not meaningful).
    fn eq(&self, o: &Known) -> bool {
        (
            self.deviation,
            self.comparator,
            self.cause,
            self.with,
            self.detail,
            self.fields,
            self.fails_with,
            self.max_delta,
            self.edit_end,
            self.cascade,
            self.when.is_some(),
            (self.degenerate, self.anywhere),
        ) == (
            o.deviation,
            o.comparator,
            o.cause,
            o.with,
            o.detail,
            o.fields,
            o.fails_with,
            o.max_delta,
            o.edit_end,
            o.cascade,
            o.when.is_some(),
            (o.degenerate, o.anywhere),
        )
    }
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
    when: None,
    degenerate: false,
    anywhere: false,
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
    const fn anywhere(self) -> Known {
        Known {
            anywhere: true,
            ..self
        }
    }
    const fn degenerate(self) -> Known {
        Known {
            degenerate: true,
            ..self
        }
    }
    const fn when(self, when: fn(&str) -> bool) -> Known {
        Known {
            when: Some(when),
            ..self
        }
    }

    /// Whether `statement` is this entry's cause.
    fn is_cause(&self, statement: &str) -> bool {
        pattern_matches(self.cause, statement) && self.when.is_none_or(|f| f(statement))
    }
}

const D_74: &str = "D-74";
const D_111: &str = "D-111";
const D_168: &str = "D-168";
const POLL: &str = "poll() failed";
const INCORRECT: &str = "context is incorrect";
const TORUS: &str = "§2 `mesh.primitive_torus_add` while a mesh is in Edit Mode";
const SPHERE_SLOTS: &str =
    "§2 `mesh.primitive_uv_sphere_add` / `primitive_ico_sphere_add` in Edit Mode after elements";
const QSORT: &str = "§2 Ties in the C library `qsort` outside `uv.smart_project`";
const SNAP: &str =
    "§2 Differential fuzzing (WP 8.7), unimplemented modes: `view3d.snap_cursor_to_selected`";
const TRANSFORM_OBJECT: &str =
    "§2 Differential fuzzing (WP 8.7), unimplemented modes: `transform.mirror`";
const POLLS: &str = "§2 Differential fuzzing (WP 8.7), operator polls and outcomes";
const GEOMETRY: &str = "§2 Differential fuzzing (WP 8.7), geometry differences";
const UV: &str = "§2 Differential fuzzing (WP 8.7), UV results";
const FOUND: &str = "§2 Differential fuzzing (WP 8.7), causal triage";

/// Statements that give an object a SUBSURF modifier (`make_links_data(type='MODIFIERS')`
/// copies the active object's).
const SUBSURF_MAKERS: &str = "SUBSURF|subdivision_set|make_links_data&type='MODIFIERS'";

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

/// `transform.shear` with a singular shear frame: its effective `orient_axis_ortho` is its
/// `orient_axis` (`initShear` resets equal axes to Z and Y, then the axes that were given
/// are read again: both given and equal, only `orient_axis_ortho='Z'`, or only
/// `orient_axis='Y'`).
fn shear_frame_is_singular(statement: &str) -> bool {
    let arg = |k: &str| {
        statement
            .split(&format!("{k}='"))
            .nth(1)
            .and_then(|r| r.chars().next())
    };
    match (arg("orient_axis"), arg("orient_axis_ortho")) {
        (Some(a), Some(o)) => a == o,
        (None, Some(o)) => o == 'Z',
        (Some(a), None) => a == 'Y',
        (None, None) => false,
    }
}

/// Operators that leave zero-area faces (walls of an extrusion that has not moved yet,
/// offset loops on top of their source).
const ZERO_AREA_MAKERS: &str = "mesh.extrude_faces_indiv|mesh.extrude_region(|mesh.extrude_context(\
     |mesh.extrude_edges_indiv|mesh.extrude_verts_indiv|mesh.offset_edge_loops(";

/// A zero-thickness inset without depth (the default `thickness=0.0`): zero-area rims.
const ZERO_THICKNESS_INSET: &str =
    "mesh.inset(&!thickness=&!depth=|mesh.inset(&thickness=0.0,&!depth=|mesh.inset(&thickness=0.0)";

/// `mesh.inset` with a non-zero `depth`.
fn inset_has_depth(statement: &str) -> bool {
    statement
        .split("depth=")
        .nth(1)
        .and_then(|r| r.split([',', ')']).next())
        .and_then(|v| v.trim().parse::<f64>().ok())
        .is_some_and(|v| v != 0.0)
}

/// `mesh.subdivide` with a non-zero `fractal`.
fn subdivide_is_fractal(statement: &str) -> bool {
    statement
        .split("fractal=")
        .nth(1)
        .and_then(|r| r.split([',', ')']).next())
        .and_then(|v| v.trim().parse::<f64>().ok())
        .is_some_and(|v| v != 0.0)
}

/// `mesh.subdivide` with a smoothing or fractal displacement (both amplify the input's
/// last-ulp differences through the vertex normals).
fn subdivide_displaces(statement: &str) -> bool {
    let arg = |k: &str| {
        statement
            .split(k)
            .nth(1)
            .and_then(|r| r.split([',', ')']).next())
            .and_then(|v| v.trim().parse::<f64>().ok())
            .is_some_and(|v| v != 0.0)
    };
    arg("fractal=") || arg("smoothness=")
}

/// Operators that change the topology (and so interpolate corner data).
const TOPOLOGY: &str = "mesh.dissolve_|mesh.unsubdivide|mesh.merge(|mesh.remove_doubles\
     |mesh.edge_split|mesh.delete|mesh.quads_convert_to_tris|mesh.tris_convert_to_quads\
     |mesh.subdivide|mesh.beautify_fill|mesh.edge_rotate|mesh.decimate|mesh.inset\
     |mesh.vert_connect|mesh.split(|mesh.extrude_|mesh.bevel|mesh.poke|mesh.bridge_edge_loops\
     |mesh.fill|mesh.spin|mesh.solidify|mesh.wireframe|mesh.duplicate|mesh.offset_edge_loops\
     |mesh.edge_face_add|mesh.triangulate|mesh.rip|mesh.bisect|mesh.knife|mesh.edge_collapse";

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
    mesh(QSORT, "mesh.face_split_by_edges", ANY),
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
    gap(POLLS, "object.vertex_group_", "no vertex groups").with(&["object.modifier_apply"]),
    line(POLLS, "object.vertex_group_normalize(", "CANCELLED"),
    line(POLLS, "object.align", "UnboundLocalError"),
    state(POLLS, RNA, "uv.lightmap_pack", &["mode", "select_mode"])
        .fails_with("No mesh object")
        .cascade(),
    // After that failure Blender is in Object Mode, where proportional editing also moves
    // unselected objects near the selection; frap transforms the edit mesh.
    state(POLLS, RNA, "uv.lightmap_pack", OBJECT_TRANSFORM)
        .fails_with("No mesh object")
        .with(&["bpy.ops.transform.&use_proportional_edit=True"])
        .anywhere(),
    gap(POLLS, "bpy.ops.transform.", "locked").with(&["shape_key_lock"]),
    line(POLLS, "object.transform_apply", "aborting"),
    // --- geometry differences ---
    mesh(GEOMETRY, "mesh.loop_to_region", ANY),
    mesh(GEOMETRY, "mesh.flatten", ANY),
    mesh(GEOMETRY, "mesh.vert_connect_nonplanar", ANY),
    // (Faces wound the other way: whatever later follows the normals differs too.)
    mesh(GEOMETRY, "mesh.extrude_edges_", ANY).with(&["mode_set(mode='EDIT')"]),
    mesh(GEOMETRY, "mesh.extrude_context", ANY).with(&["mode_set(mode='EDIT')", "mesh.primitive_"]),
    state(
        GEOMETRY,
        RNA,
        "ORIGIN_CENTER_OF_VOLUME",
        &["location", "matrix"],
    ),
    mesh(GEOMETRY, "ORIGIN_CENTER_OF_VOLUME", POSITIONS),
    // An edge attribute with one value on every edge is a single-value attribute in Blender,
    // which SUBSURF gives to its inner edges too (frap: the default); a second or applied
    // SUBSURF then creases otherwise. In Edit Mode (`edit_end`) the result also has other
    // edges after some extrusions.
    mesh(
        GEOMETRY,
        "transform.edge_crease|shade_smooth_by_angle|mesh.mark_sharp|mesh.split_normals\
         |mark_seam|transform.edge_bevelweight|mesh.set_sharpness_by_angle\
         |uv.seams_from_islands",
        EDIT_EDGE_DATA,
    )
    .with(&[SUBSURF_MAKERS]),
    mesh(
        GEOMETRY,
        "geometry.attribute_add&domain='EDGE'",
        &["attributes"],
    )
    .with(&[SUBSURF_MAKERS]),
    mesh(GEOMETRY, "transform.edge_crease", ANY).with(&[
        SUBSURF_MAKERS,
        "object.modifier_copy|object.modifier_apply|object.convert",
    ]),
    // `primitive_torus_add` builds a mesh whose active face is 0 (`bpy.data.meshes.new`):
    // the `ACTIVE_ELEMENT` pivot in Edit Mode is face 0 in Blender, the median in frap.
    mesh(GEOMETRY, "mesh.primitive_torus_add", POSITIONS).with(&[
        "transform_pivot_point = 'ACTIVE_ELEMENT'",
        "mode_set(mode='EDIT')",
    ]),
    mesh(
        GEOMETRY,
        "mesh.extrude_edges_indiv",
        &["edges", "faces", ""],
    )
    .with(&[SUBSURF_MAKERS]),
    mesh(GEOMETRY, "mesh.extrude_region|mesh.extrude_context", ANY).with(&[SUBSURF_MAKERS]),
    mesh(GEOMETRY, "object.transform_apply", POSITIONS).with(&["shape_key_add", SUBSURF_MAKERS]),
    // A few units of the custom normals' 16-bit encoding.
    mesh(GEOMETRY, CUSTOM_NORMALS, &["attributes[\"custom_normal\"]"]).max_delta(8.0),
    mesh(GEOMETRY, "mesh.dissolve_mode&use_face_split=True", NORMALS).with(&["mesh.normals_tools"]),
    mesh(GEOMETRY, "mesh.edge_face_add", ANY).with(&["mode_set(mode='EDIT')"]),
    mesh(GEOMETRY, "mesh.decimate", ANY),
    // SUBSURF positions are T2: the many cost ties of a subdivided symmetric or flat mesh flip
    // (the same `BM_mesh_decimate_dissolve_ex` as DECIMATE's `DISSOLVE`; exact on Blender's
    // positions: `fuzz/dissolve_limited_subsurf_circle_ties.py`).
    mesh("D-120", "mesh.dissolve_limited", ANY).with(&[SUBSURF_MAKERS]),
    mesh(GEOMETRY, "uv.align", UVS),
    mesh(GEOMETRY, "mesh.fill(", ANY),
    mesh(GEOMETRY, "BEVEL", UVS),
    mesh(GEOMETRY, "mesh.edge_split", NORMALS).with(&["mesh.merge_normals"]),
    mesh(GEOMETRY, "mesh.edge_rotate", ANY).with(&["mode_set(mode='EDIT')"]),
    mesh(GEOMETRY, "mesh.normals_make_consistent", ANY).with(&[
        "mode_set(mode='EDIT')",
        "mesh.primitive_plane_add|mesh.primitive_grid_add|mesh.primitive_circle_add",
    ]),
    // (Positions of the added geometry, and whatever the modifiers derive from them.)
    mesh(GEOMETRY, "mesh.primitive_", ANY).with(&["shape_key_add", "mode_set(mode='EDIT')"]),
    mesh(GEOMETRY, "uv.weld", UVS).with(&["mesh.split"]),
    mesh(GEOMETRY, "mesh.convex_hull", ANY),
    mesh(
        GEOMETRY,
        "~bpy.ops.mesh.",
        &["attributes[\"material_index\"]"],
    )
    .with(&["material_slot_add"]),
    mesh(GEOMETRY, "mesh.quads_convert_to_tris", ANY),
    mesh(
        GEOMETRY,
        "object.transform_apply|object.origin_set",
        POSITIONS,
    )
    .with(&["shape_key_add"]),
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
    // Blender creates the side faces of an extrusion in pointer-hash order (D-65): a later
    // dissolve that joins them winds the merged face by that order.
    mesh("D-65", "mesh.dissolve_", WINDING)
        .with(&["mesh.extrude_repeat|mesh.extrude_region|mesh.extrude_context"]),
    // A vertex split after such an extrusion gives each wall one of the coincident copies in
    // the walls' order: the same mesh up to which coincident vertex each wall uses (exact on
    // Blender's extruded mesh: `fuzz/edge_split_vert_after_extrude_ties.py`).
    mesh("D-65", "mesh.edge_split", ANY)
        .with(&["mesh.extrude_repeat|mesh.extrude_region|mesh.extrude_context"]),
    // Blender's own UV sphere element order changes between runs (D-19): results that
    // depend on the element order (index writes, ties) are not comparable on its mesh.
    mesh("D-19", "mesh.primitive_uv_sphere_add", ANY),
    // --- found by the causal triage (WP 8.7 fix round) ---
    mesh(FOUND, "transform.shear", NORMALS).with(&["mesh.subdivide", "mesh.faces_shade_smooth"]),
    mesh(FOUND, "mesh.bevel", ANY).with(&["mesh.extrude_faces_indiv|mesh.wireframe"]),
    mesh(FOUND, "mesh.bridge_edge_loops", ANY),
    mesh(FOUND, "mesh.extrude_region", NORMALS).with(&["mesh.dissolve_edges"]),
    mesh(FOUND, "transform.tosphere", NORMALS).with(&["mesh.extrude_faces_indiv"]),
    mesh(FOUND, "transform.shear", ANY)
        .with(&["mode_set(mode='EDIT')"])
        .edit_end(),
    mesh(FOUND, "mesh.dissolve_", NORMALS).with(&["mesh.set_sharpness_by_angle|mesh.mark_sharp"]),
    state(FOUND, RNA, "object.align", OBJECT_TRANSFORM).with(&[SUBSURF_MAKERS]),
    mesh(FOUND, TOPOLOGY, NORMALS).with(&[CUSTOM_NORMALS]),
    // `set_sharpness_by_angle` adds a (zero) `custom_normal` layer in Blender 5.2: topology
    // operators after it keep Blender's custom normals as they do after the tools above.
    mesh(FOUND, TOPOLOGY, NORMALS).with(&["mesh.set_sharpness_by_angle"]),
    mesh(FOUND, "mesh.solidify", POSITIONS).with(&["mesh.dissolve_edges"]),
    mesh(
        FOUND,
        "transform.shrink_fatten&use_even_offset=True",
        POSITIONS,
    ),
    // An inset corner with a huge offset factor (nearly opposite edges on Suzanne's boundary)
    // amplifies the input's rounding past the case tolerance.
    mesh(FOUND, "mesh.inset", POSITIONS).with(&["primitive_monkey_add"]),
    mesh(FOUND, "transform.shear", ANY).when(shear_frame_is_singular),
    mesh(FOUND, "mesh.tris_convert_to_quads", ANY).with(&["TRIFAN"]),
    // Join errors that tie up to float rounding on the symmetric triangles of an ico sphere,
    // a poked torus or a fan.
    mesh(FOUND, "mesh.tris_convert_to_quads", ANY)
        .with(&["primitive_ico_sphere_add|primitive_torus_add|mesh.poke"]),
    // Spheres added in Edit Mode after a deletion: other element order (§2), so order-dependent
    // tools that follow differ.
    mesh(
        SPHERE_SLOTS,
        "mesh.primitive_ico_sphere_add|mesh.primitive_uv_sphere_add",
        ANY,
    )
    .with(&["~mesh.delete|mesh.dissolve_|mesh.merge(|mesh.remove_doubles"]),
    mesh(FOUND, "use_boundary_tear=True", ANY),
    mesh(FOUND, "transform.push_pull|transform.tosphere", POSITIONS)
        .with(&["mode_set(mode='EDIT')"]),
    gap(D_111, "bpy.ops.uv.", "frap CANCELLED != blender ok").with(&["bpy.ops.uv.select"]),
    // A UV selection operator after `uv.select_split` (which may leave Blender's UV
    // selection partly independent of the mesh selection even when it cancels).
    mesh(D_111, "bpy.ops.uv.", ANY).with(&["~uv.select_split"]),
    gap(D_111, "bpy.ops.uv.", "frap ok != blender CANCELLED").with(&["~uv.select_split"]),
    // `uv.select_linked` / `uv.select_pinned` leave Blender's UV selection partly independent
    // of the mesh selection; a later UV operator, or `uv.select_all(action='INVERT')`, then
    // starts from another selection.
    gap(D_111, "bpy.ops.uv.", "frap ok != blender CANCELLED")
        .with(&["uv.select_linked|uv.select_pinned"]),
    // A second `uv.select_linked` after a topology operator (`wireframe`) starts from
    // Blender's surviving partial UV selection: other mesh selection, and a mesh operator
    // after it finds nothing selected in Blender.
    mesh(
        D_111,
        "bpy.ops.uv.select_linked",
        &["selection", "active", "select_history"],
    )
    .with(&["uv.select_linked|uv.select_pinned", "bpy.ops.mesh."]),
    line(D_111, "bpy.ops.mesh.", "frap ok != blender CANCELLED").with(&[
        "uv.select_linked|uv.select_pinned",
        "bpy.ops.mesh.wireframe",
    ]),
    mesh(
        D_111,
        "uv.select_all&INVERT",
        &["selection", "active", "select_history"],
    )
    .with(&["uv.select_linked|uv.select_pinned"]),
    mesh(FOUND, "BEVEL", &["corner_normals", "face_normals"]).max_delta(1e-3),
    mesh(FOUND, "mesh.bevel&affect='VERTICES'", UVS),
    mesh(
        FOUND,
        "mesh.bevel",
        &["corner_normals", "face_normals", "uv_layers"],
    )
    .max_delta(2e-3),
    mesh(FOUND, "TRIANGULATE", ANY),
    mesh(FOUND, "mesh.primitive_", UVS).with(&["mode_set(mode='EDIT')", TOPOLOGY]),
    mesh(FOUND, "mesh.unsubdivide", WINDING).with(&["mesh.extrude_region"]),
    mesh(FOUND, "mesh.separate", &["attributes[\"__mirror_index\"]"]).with(&["mesh.select_mirror"]),
    mesh(FOUND, "mesh.remove_doubles", ANY),
    // (The selection differs: whatever acts on it next differs too.)
    mesh(FOUND, "~mesh.edge_face_add", ANY).with(&["mesh.region_to_loop"]),
    mesh(FOUND, "mesh.select_random", ANY).with(&[TOPOLOGY]),
    mesh(FOUND, "uv.randomize_uv_transform", UVS).with(&[TOPOLOGY]),
    mesh(FOUND, "mesh.vert_connect", NORMALS).with(&["mesh.sort_elements"]),
    mesh(FOUND, "mesh.duplicate", ANY).with(&["mesh.wireframe"]),
    mesh(FOUND, "WELD", WINDING).with(&["mesh.edge_split"]),
    gap(
        POLLS,
        "mesh.reorder_vertices_spatial",
        "frap ok != blender CANCELLED",
    )
    .with(&["mesh.primitive_"]),
    mesh(FOUND, "mesh.offset_edge_loops", NORMALS).with(&["mesh.poke"]),
    state(
        POLLS,
        RNA,
        "object.parent_no_inverse_set",
        &["parent", "location", "rotation", "scale", "matrix"],
    )
    .with(&["object.select_pattern|object.select_all"]),
    mesh(FOUND, "MIRROR", &["edges", "faces", ""]).with(&[SUBSURF_MAKERS]),
    mesh(FOUND, "mesh.spin", NORMALS).with(&["mesh.extrude_faces_indiv"]),
    // `bevel_reattach_wires` picks the closest of two new vertices equidistant from a
    // zero-length wire's other end: float rounding of the (T2) input decides (exact on
    // Blender's positions: `fuzz/bevel_modifier_wire_reattach_ties.py`).
    mesh(FOUND, "BEVEL", &["edges"]).with(&["mesh.extrude_verts_indiv"]),
    // `symmetry_snap` pairs each vertex with its nearest mirror candidate within the
    // threshold: near-equidistant candidates after a duplicating `spin` (T2 positions) pair
    // otherwise (exact on Blender's positions: `fuzz/symmetry_snap_after_spin_ties.py`).
    mesh(FOUND, "mesh.symmetry_snap", POSITIONS).with(&["mesh.spin"]),
    // A BEVEL over a MIRROR seam of SUBSURF vertices a few 1e-9 off the plane: which
    // diagonal the bevel patches depends on those (T2) last ulps (exact on Blender's
    // positions: `fuzz/bevel_mirror_subsurf_seam_ties.py`).
    mesh(FOUND, "BEVEL", ANY).with(&[SUBSURF_MAKERS, "MIRROR"]),
    // DISPLACE over SUBSURF folds thin faces whose normals amplify the (T2) SUBSURF positions
    // past 2e-3 (exact on Blender's positions: `fuzz/displace_subsurf_suzanne_ties.py`).
    mesh(FOUND, "DISPLACE", &["corner_normals", "face_normals"]).with(&[SUBSURF_MAKERS]),
    // `subdivide` smoothing or fractal displacement amplifies the spun positions' last-ulp
    // differences (exact on Blender's positions: `fuzz/subdivide_fractal_after_spin_ties.py`).
    mesh(FOUND, "mesh.subdivide", POSITIONS)
        .when(subdivide_displaces)
        .with(&["mesh.spin"]),
    // `dissolve_limited` of a flat mesh: its costs (face angles and vertex angles near zero)
    // tie up to the rounding of positions a rotation left T2 (exact on Blender's positions).
    mesh(FOUND, "mesh.dissolve_limited", ANY).with(&[
        "bpy.ops.transform.rotate",
        "primitive_grid_add|primitive_plane_add|primitive_circle_add",
    ]),
    mesh(
        FOUND,
        "mesh.select_mirror",
        &["selection", "active", "select_history"],
    )
    .with(&["mesh.spin"]),
    // Selected edges whose vertices are not selected (vertex extrusion in edge mode):
    // Blender reads their vertices' mirrors from its temporary `__mirror_index` layer (0 where
    // never written), frap finds none.
    mesh(
        FOUND,
        "mesh.select_mirror",
        &["selection", "active", "select_history"],
    )
    .with(&["mesh.extrude_verts_indiv|mesh.extrude_vertices_move"]),
    // The cursor follows an object `object.align` placed otherwise (its SUBSURF bounds).
    state(FOUND, RNA, "view3d.snap_cursor_to_", &["cursor"])
        .with(&["object.align", SUBSURF_MAKERS]),
    mesh(FOUND, "bpy.ops.transform.rotate", NORMALS).with(&[CUSTOM_NORMALS]),
    // Edit Mode transforms other than translate/rotate/resize (no `preserve_clnor`) leave
    // Blender's corner spaces stale, so `flip_normals` decodes the custom normals in the
    // spaces of the untransformed geometry (frap: the current ones; M9).
    mesh(FOUND, "mesh.flip_normals", NORMALS).with(&[
        CUSTOM_NORMALS,
        "transform.push_pull|transform.shrink_fatten|transform.tosphere|transform.shear\
         |transform.bend|transform.vertex_random|transform.vert_slide|transform.edge_slide",
    ]),
    mesh(FOUND, "transform.shrink_fatten", POSITIONS)
        .with(&["mesh.extrude_verts_indiv|mesh.extrude_vertices_move"]),
    mesh(FOUND, "mesh.delete_edgeloop", ANY).with(&["mesh.extrude_edges_indiv"]),
    mesh(
        FOUND,
        "geometry.attribute_remove|geometry.attribute_convert",
        ANY,
    )
    .with(&["geometry.attribute_add", "object.shade_"]),
    // Normals of thin faces: float accumulation (positions agree at the case tolerance).
    mesh(
        FOUND,
        "bpy.ops.mesh.|bpy.ops.transform.|bpy.ops.object.modifier_add|modifiers.new\
         |bpy.ops.object.subdivision_set",
        &["corner_normals", "face_normals"],
    )
    .max_delta(2e-3),
    mesh(FOUND, "SCREW", WINDING),
    mesh(FOUND, "WEIGHTED_NORMAL", NORMALS).max_delta(1e-3),
    mesh(FOUND, "WEIGHTED_NORMAL", &["attributes[\"custom_normal\"]"]).max_delta(4.0),
    mesh(FOUND, "BEVEL", ANY).with(&["mesh.primitive_torus_add"]),
    mesh(FOUND, "mesh.merge", NORMALS).with(&["mesh.extrude_region"]),
    // CAST normalises each vertex's direction from the centre: SUBSURF's T2 positions of a
    // vertex at the centre (about 1e-8 from it) point it elsewhere.
    mesh(FOUND, "CAST", POSITIONS).with(&[SUBSURF_MAKERS]),
    // SCREW treats a vertex on its axis apart: SUBSURF's T2 face centres on the axis (exactly
    // on it in frap, about 1e-8 off in Blender) are screwed otherwise.
    mesh(FOUND, "SCREW", ANY).with(&[SUBSURF_MAKERS]),
    mesh(FOUND, "LAPLACIANSMOOTH", POSITIONS)
        .with(&["primitive_plane_add|primitive_grid_add|primitive_circle_add"]),
    mesh(FOUND, CUSTOM_NORMALS, NORMALS).with(&["transform.shear"]),
    // In-plane extrusion of a flat mesh: overlapping faces of opposite normals whose sum
    // cancels at the smooth corners, in the (pointer-hash, D-65) order of Blender's side faces.
    mesh(FOUND, "mesh.extrude_region_move", NORMALS).with(&[
        "shade_smooth",
        "primitive_grid_add|primitive_plane_add|primitive_circle_add",
    ]),
    // Zero-area faces: their normals (and the smooth corners around them) are decided by
    // float rounding, whichever operator made or last moved them.
    mesh(FOUND, "~bpy.ops.", &["corner_normals", "face_normals"]).degenerate(),
    // `subdivide(fractal=…)` displaces the new vertices along vertex normals, which zero-area
    // walls (extruded but not moved) feed.
    mesh(FOUND, "mesh.subdivide", POSITIONS)
        .when(subdivide_is_fractal)
        .with(&[ZERO_AREA_MAKERS]),
    // `inset(depth=…)` moves the inset along vertex normals: positions differ when the mesh
    // has zero-area faces (a zero-thickness inset before, the default `thickness=0.0`, leaves
    // zero-area rims).
    mesh(FOUND, "mesh.inset", POSITIONS)
        .when(inset_has_depth)
        .with(&[ZERO_THICKNESS_INSET]),
    // `BM_mesh_region_match` reads an uninitialised UID for vertices inside n-gons of five or
    // more sides (D-168); what later operators do with that selection differs too.
    mesh(D_168, "mesh.select_similar_region", ANY).with(&[
        "mesh.symmetrize|mesh.bisect|mesh.dissolve_|fill_type='NGON'\
         |primitive_cone_add&!end_fill_type=|primitive_cylinder_add&!end_fill_type=",
    ]),
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
            let ok = statements.get(i).is_some_and(|s| self.is_cause(s))
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
        if self.max_delta > 0.0 && delta(&m.detail).is_none_or(|d| periodic(d) > self.max_delta) {
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
        if self.degenerate {
            let key = m.path.split(']').next().map(|k| format!("{k}]"));
            let covered = key
                .and_then(|k| trace.degenerate.get(&k))
                .is_some_and(|d| d.covers(field));
            if !covered {
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
        // The first statement that can be the cause (what follows it may be its consequence).
        let cause = (0..statements.len()).find(|&i| {
            self.is_cause(&statements[i]) && ran(i) && (self.anywhere || trace.touches(i, &t))
        })?;
        let with = self.with.iter().all(|p| {
            (0..statements.len()).any(|j| {
                trace.ran(j, p)
                    && pattern_matches(p, &statements[j])
                    && (self.anywhere || trace.touches(j, &t))
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
            // The entry with the earliest cause (the first entry on a tie).
            KNOWN
                .iter()
                .filter_map(|k| k.explains(statements, trace, m).map(|i| (k, i)))
                .min_by_key(|(_, i)| *i)
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
                (failed && k.is_cause(&statements[i])).then_some((k, i))
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
            degenerate: BTreeMap::new(),
        }
    }

    /// M8 stabilization round 1: normals of zero-area faces (and the smooth corners around
    /// them) are explained, other corners of the same mesh are not.
    #[test]
    fn zero_area_faces_explain_their_normals_only() {
        let mut m = crate::state::GMesh {
            positions: Vec::new(),
            edges: Vec::new(),
            faces: Vec::new(),
            corner_normals: Vec::new(),
            face_normals: None,
            uv_layers: BTreeMap::new(),
            attributes: BTreeMap::new(),
            selection: serde_json::Value::Null,
            active: serde_json::Value::Null,
            select_history: serde_json::Value::Null,
        };
        // Face 0: a unit quad (z = 0); face 1: a twisted quad within z = 1 (zero area); face
        // 2: a flat quad sharing no vertex with face 1.
        m.positions = vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [1.0, 1.0, 1.0],
            [-1.0, 1.0, 1.0],
            [1.0, -1.0, 1.0],
            [-1.0, -1.0, 1.0],
            [5.0, 0.0, 0.0],
            [6.0, 0.0, 0.0],
            [6.0, 1.0, 0.0],
            [5.0, 1.0, 0.0],
        ];
        m.faces = vec![vec![0, 1, 2, 3], vec![4, 5, 6, 7], vec![8, 9, 10, 11]];
        let d = Degenerate::of(&m);
        assert_eq!(d.faces, BTreeSet::from([1]));
        assert_eq!(d.corners, BTreeSet::from([4, 5, 6, 7]));
        let statements = st(&["bpy.ops.object.mode_set(mode='EDIT')"]);
        let mut trace = cube_trace(1);
        trace.degenerate.insert("meshes[\"Cube\"]".into(), d);
        let mm = |path: &str| {
            Mismatch::new(
                MESH,
                path,
                "frap [0.0, 0.0, -1.0] != blender [0.0, 0.0, 1.0]",
            )
        };
        let r = explain_all(
            &statements,
            &trace,
            &[
                mm("meshes[\"Cube\"].corner_normals[5]"),
                mm("meshes[\"Cube\"].face_normals[1]"),
            ],
        );
        assert!(r.iter().all(|k| k.is_some_and(|k| k.degenerate)), "{r:?}");
        let r = explain_all(
            &statements,
            &trace,
            &[mm("meshes[\"Cube\"].corner_normals[9]")],
        );
        assert_eq!(r, vec![None]);
        assert!(subdivide_is_fractal(
            "bpy.ops.mesh.subdivide(number_cuts=1, fractal=0.45, seed=23)"
        ));
        assert!(!subdivide_is_fractal("bpy.ops.mesh.subdivide(fractal=0.0)"));
        assert!(!subdivide_is_fractal(
            "bpy.ops.mesh.subdivide(number_cuts=2)"
        ));
        assert!(pattern_matches(
            ZERO_THICKNESS_INSET,
            "bpy.ops.mesh.inset(use_boundary=True)"
        ));
        assert!(!pattern_matches(
            ZERO_THICKNESS_INSET,
            "bpy.ops.mesh.inset(depth=-0.25)"
        ));
        assert!(!pattern_matches(
            ZERO_THICKNESS_INSET,
            "bpy.ops.mesh.inset(thickness=0.1)"
        ));
        assert!(inset_has_depth(
            "bpy.ops.mesh.inset(depth=-0.25, use_outset=True)"
        ));
    }

    /// M8 stabilization round 2: a triangle collapsed onto a point (the corner a BEVEL closes
    /// after another BEVEL: every vertex within rounding of the others) is degenerate.
    #[test]
    fn collapsed_faces_are_degenerate() {
        let m = crate::state::GMesh {
            positions: vec![
                [-1.0, -0.5, -0.75],
                [-1.0, -0.500_000_06, -0.75],
                [-1.000_000_1, -0.5, -0.75],
                [0.0, 0.0, 0.0],
                [1e-6, 0.0, 0.0],
                [0.0, 1e-6, 0.0],
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
            ],
            edges: Vec::new(),
            faces: vec![vec![0, 1, 2], vec![3, 4, 5], vec![6, 7, 8]],
            corner_normals: Vec::new(),
            face_normals: None,
            uv_layers: BTreeMap::new(),
            attributes: BTreeMap::new(),
            selection: serde_json::Value::Null,
            active: serde_json::Value::Null,
            select_history: serde_json::Value::Null,
        };
        let d = Degenerate::of(&m);
        assert_eq!(d.faces, BTreeSet::from([0, 1]));
    }

    /// M8 stabilization round 2: two coincident quads of opposite winding (an in-plane
    /// extrusion): the smooth normals of their shared vertices cancel; a lone quad's do not.
    #[test]
    fn cancelling_vertex_normals_are_degenerate() {
        let quad = |o: f32| {
            vec![
                [o, 0.0, 0.0],
                [o + 1.0, 0.0, 0.0],
                [o + 1.0, 1.0, 0.0],
                [o, 1.0, 0.0],
            ]
        };
        let mut positions = quad(0.0);
        positions.extend(quad(5.0));
        let m = crate::state::GMesh {
            positions,
            edges: Vec::new(),
            faces: vec![vec![0, 1, 2, 3], vec![3, 2, 1, 0], vec![4, 5, 6, 7]],
            corner_normals: Vec::new(),
            face_normals: None,
            uv_layers: BTreeMap::new(),
            attributes: BTreeMap::new(),
            selection: serde_json::Value::Null,
            active: serde_json::Value::Null,
            select_history: serde_json::Value::Null,
        };
        let d = Degenerate::of(&m);
        assert!(d.faces.is_empty());
        assert_eq!(d.corners, (0..8).collect());
    }

    /// M8 stabilization round 2: the fuzz cases whose causes are float ties of T2 inputs.
    #[test]
    fn round_two_ties_are_explained() {
        let explained = |statements: &[&str], path: &str, detail: &str| {
            let statements = st(statements);
            let trace = cube_trace(statements.len());
            explain_all(&statements, &trace, &[Mismatch::new(MESH, path, detail)])[0]
                .map(|k| k.deviation)
        };
        assert_eq!(
            explained(
                &[
                    "bpy.ops.object.subdivision_set()",
                    "bpy.ops.object.modifier_apply(modifier='Subdivision')",
                    "bpy.ops.object.mode_set(mode='EDIT')",
                    "bpy.ops.mesh.dissolve_limited(angle_limit=0.59, delimit={'NORMAL', 'SEAM'})",
                ],
                "meshes[\"Cube\"]",
                "frap (verts, edges, faces) (7, 7, 1) != blender (8, 8, 1)",
            ),
            Some("D-120")
        );
        assert_eq!(
            explained(
                &[
                    "bpy.ops.object.mode_set(mode='EDIT')",
                    "bpy.ops.mesh.dissolve_limited(angle_limit=0.59)",
                ],
                "meshes[\"Cube\"]",
                "frap (verts, edges, faces) (7, 7, 1) != blender (8, 8, 1)",
            ),
            None
        );
        assert_eq!(
            explained(
                &[
                    "bpy.ops.object.subdivision_set(level=2)",
                    "bpy.ops.object.mode_set(mode='EDIT')",
                    "bpy.ops.mesh.extrude_context(use_normal_flip=False)",
                ],
                "evaluated[\"Cube\"].edges[0]",
                "frap edge [5, 16] has no blender counterpart",
            ),
            Some(GEOMETRY)
        );
        assert_eq!(
            explained(
                &[
                    "bpy.context.object.modifiers.new('Sub', 'BEVEL')",
                    "bpy.ops.object.mode_set(mode='EDIT')",
                    "bpy.ops.mesh.spin(center=(-0.84, 0.09, -0.88), axis=(1, 0, 0))",
                    "bpy.ops.mesh.extrude_verts_indiv()",
                ],
                "evaluated[\"Cube\"].edges[626]",
                "frap edge [303, 2] has no blender counterpart",
            ),
            Some(FOUND)
        );
        assert_eq!(
            explained(
                &[
                    "bpy.ops.object.mode_set(mode='EDIT')",
                    "bpy.ops.mesh.spin(steps=3, axis=(1, 0, 0), dupli=True)",
                    "bpy.ops.mesh.symmetry_snap(threshold=0.37, use_center=False)",
                ],
                "meshes[\"Cube\"].positions[86]",
                "frap vertex [-0.27638802, -2.1175513, 1.6128819] has no blender counterpart",
            ),
            Some(FOUND)
        );
    }

    #[test]
    fn round_three_ties_are_explained() {
        let explained = |statements: &[&str], path: &str, detail: &str| {
            let statements = st(statements);
            let trace = cube_trace(statements.len());
            explain_all(&statements, &trace, &[Mismatch::new(MESH, path, detail)])[0]
                .map(|k| k.deviation)
        };
        // Fuzz seed 0xd261cd89f7db8a29 (run 0x8e417e57).
        assert_eq!(
            explained(
                &[
                    "bpy.ops.object.mode_set(mode='EDIT')",
                    "bpy.ops.mesh.edge_split(type='EDGE')",
                    "bpy.ops.mesh.extrude_context()",
                    "bpy.ops.mesh.edge_split(type='VERT')",
                ],
                "meshes[\"Cube\"].edges[48]",
                "frap edge [10, 51] has no blender counterpart",
            ),
            Some("D-65")
        );
        // Without the extrusion there is no wall order to differ.
        assert_eq!(
            explained(
                &[
                    "bpy.ops.object.mode_set(mode='EDIT')",
                    "bpy.ops.mesh.edge_split(type='EDGE')",
                    "bpy.ops.mesh.edge_split(type='VERT')",
                ],
                "meshes[\"Cube\"].edges[48]",
                "frap edge [10, 51] has no blender counterpart",
            ),
            None
        );
        // Fuzz seed 0xe8478421dc30ca42 (run 0x5eed8e90); a plain subdivide is not explained.
        let spin = [
            "bpy.ops.object.mode_set(mode='EDIT')",
            "bpy.ops.mesh.spin(center=(0.16, 0.72, -0.12), axis=(1, 0, 0), dupli=True)",
            "bpy.ops.mesh.subdivide(smoothness=0.85, fractal=0.68)",
        ];
        assert_eq!(
            explained(
                &spin,
                "meshes[\"Cube\"].positions[119]",
                "frap vertex [-1.5643852, 0.8221707, 0.60858494] has no blender counterpart",
            ),
            Some(FOUND)
        );
        let mut plain = spin;
        plain[2] = "bpy.ops.mesh.subdivide(number_cuts=2)";
        assert_eq!(
            explained(
                &plain,
                "meshes[\"Cube\"].positions[119]",
                "frap vertex [-1.5643852, 0.8221707, 0.60858494] has no blender counterpart",
            ),
            None
        );
        // Fuzz seed 0xcd369cec07d504cf (run 0x5eed8e90): the SUBSURF came from the active
        // object through `make_links_data`.
        assert_eq!(
            explained(
                &[
                    "bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=1, radius=1.27)",
                    "bpy.ops.object.subdivision_set(level=1)",
                    "bpy.ops.object.select_all(action='SELECT')",
                    "bpy.ops.object.make_links_data(type='MODIFIERS')",
                    "bpy.ops.object.mode_set(mode='EDIT')",
                    "bpy.ops.mesh.mark_sharp()",
                ],
                "evaluated[\"Cube\"].attributes[\"sharp_edge\"].data[3]",
                "frap false != blender true",
            ),
            Some(GEOMETRY)
        );
        // Fuzz seed 0xb2269be17581f0aa (run 0x5eed8e90).
        assert_eq!(
            explained(
                &[
                    "bpy.ops.object.subdivision_set(relative=True)",
                    "bpy.context.object.modifiers.new('Arr', 'MIRROR')",
                    "bpy.context.object.modifiers.new('Sub', 'BEVEL')",
                ],
                "evaluated[\"Cube\"].positions[72]",
                "frap vertex [0.0, 0.0, 0.9463842] has no blender counterpart",
            ),
            Some(FOUND)
        );
        // Fuzz seed 0x624c3faf39918f30 (run 0x5eed8e91).
        assert_eq!(
            explained(
                &[
                    "bpy.ops.mesh.primitive_monkey_add(size=1.3)",
                    "bpy.ops.object.subdivision_set(level=2)",
                    "bpy.ops.object.modifier_add(type='DISPLACE')",
                ],
                "evaluated[\"Cube\"].corner_normals[2176]",
                "frap [-0.38685718, 0.69868505, -0.6018146] != blender [-0.3, 0.69, -0.6]",
            ),
            Some(FOUND)
        );
        // Fuzz seed 0x368aaba5457fea3f (run 0x5eed8e91).
        assert_eq!(
            explained(
                &[
                    "bpy.ops.object.mode_set(mode='EDIT')",
                    "bpy.ops.mesh.average_normals()",
                    "bpy.ops.transform.push_pull(value=0.39)",
                    "bpy.ops.mesh.flip_normals()",
                ],
                "meshes[\"Cube\"].corner_normals[0]",
                "frap [-0.94836587, 0.25712878, 0.18570702] != blender [-0.98, 0.15, 0.01]",
            ),
            Some(FOUND)
        );
        // Fuzz seed 0xb85755713edef4b: fixed (edit-mode entry recomputes the BMesh normals),
        // so it stays unexplained.
        assert_eq!(
            explained(
                &[
                    "bpy.ops.object.mode_set(mode='EDIT')",
                    "bpy.ops.mesh.extrude_faces_indiv()",
                    "bpy.ops.object.mode_set(mode='OBJECT')",
                    "bpy.ops.object.mode_set(mode='EDIT')",
                    "bpy.ops.mesh.inset(thickness=0.33, depth=-0.15, use_even_offset=False, \
                     use_edge_rail=False, use_outset=True, use_select_inset=False)",
                ],
                "meshes[\"Cube\"].positions[36]",
                "frap vertex [1.0, -0.9329, 0.8658] has no blender counterpart",
            ),
            None
        );
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
        assert!(shear_frame_is_singular(
            "bpy.ops.transform.shear(angle=0.4, orient_axis_ortho='Z')"
        ));
        assert!(shear_frame_is_singular(
            "bpy.ops.transform.shear(angle=0.4, orient_axis='Y', orient_axis_ortho='Y')"
        ));
        assert!(!shear_frame_is_singular(
            "bpy.ops.transform.shear(angle=0.4)"
        ));
        assert!(shear_frame_is_singular(
            "bpy.ops.transform.shear(angle=0.4, orient_axis='Y')"
        ));
        assert!(!shear_frame_is_singular(
            "bpy.ops.transform.shear(angle=0.4, orient_axis='X', orient_axis_ortho='Y')"
        ));
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
        assert!(
            k.explains(&s, &tr, &m("frap 32767 != blender -32767"))
                .is_some()
        );
        assert!(
            k.explains(&s, &tr, &m("frap 100 != blender -100"))
                .is_none()
        );
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
            "bpy.ops.transform.shear(angle=0.4)",
        ]);
        let m = Mismatch::new(
            "mesh_canonical",
            "evaluated[\"Cube\"].corner_normals[3]",
            "frap [0.0, 0.0, 1.0] != blender [0.3, 0.0, 0.95]",
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
        // Unless a second (copied) or applied SUBSURF subdivides the creased inner edges.
        let mut s = s;
        s.push("bpy.ops.object.modifier_copy(modifier='Subdivision')".into());
        assert!(explain_all(&s, &cube_trace(4), std::slice::from_ref(&p))[0].is_some());
    }
}
