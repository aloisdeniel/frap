// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.frap.scene_summary(objects='ALL', evaluated=True)`, tier T0
//! (`tests/golden/cases/edit/select/scene_summary.py`): frap's one-call overview of the scene
//! for agents (ARCHITECTURE §22.4; spec `spec/frap/ops/frap.scene_summary.json`). Blender has
//! no such operator (the golden emulation is a no-op that succeeds); the operator changes
//! nothing and answers in `result`:
//!
//! - `scene`, `frame_current`, `mode` (`context.mode`), `active_object` (a reference or null);
//! - `objects`: one entry per object of the context view layer, in `view_layer.objects` order,
//!   filtered by `objects` (`ALL`, `VISIBLE`: `visible_get()`, `SELECTED`: `select_get()` and
//!   visible). Each entry has `name`, `type`, `data`, `parent`, `mode`, `select`, `visible`,
//!   `location`, `rotation_euler`, `scale`, `materials` (slot material names, `""` for an
//!   empty slot) and `modifiers` (`name`, `type`, `show_viewport`); a mesh object also has
//!   `vertices`, `edges`, `faces`, `triangles` and its world-space bounding box `bounds_min`
//!   / `bounds_max` (from the 8 corners of the local box, as `bound_box` and `matrix_world`
//!   give them). With `evaluated` (the default) counts and bounds come from the evaluated
//!   mesh (modifiers applied), else from the mesh data. A mesh in Edit Mode is summarised from
//!   its edit mesh (what `mesh.vertices` reads in Edit Mode);
//! - `vertices`, `edges`, `faces`, `triangles`: totals over the listed mesh objects;
//! - `materials`: the names of the materials used by the listed objects, in order of first use.

use frap_bmesh::BMesh;
use frap_mesh::Mesh;
use frap_op::operator;
use frap_op::prelude::*;
use frap_rna::{ArrayShape, IdRef, MaterialId, MathTag, MeshId, ObjectId};
use frap_scene::id::slots;
use frap_scene::{Main, ObjectMode};
use frap_spec_ops_frap::frap::{self, scene_summary::Objects};
use glam::{Mat4, Vec3};
use indexmap::IndexMap;
use smallvec::SmallVec;
use smol_str::SmolStr;

/// Element counts of a mesh.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Counts {
    pub(crate) vertices: usize,
    pub(crate) edges: usize,
    pub(crate) faces: usize,
    pub(crate) triangles: usize,
}

impl Counts {
    fn add(&mut self, o: Counts) {
        self.vertices += o.vertices;
        self.edges += o.edges;
        self.faces += o.faces;
        self.triangles += o.triangles;
    }
}

/// Counts and local bounds of a mesh.
pub(crate) fn mesh_stats(me: &Mesh) -> (Counts, Option<[[f32; 3]; 2]>) {
    let offsets = me.face_offsets();
    let triangles = offsets
        .windows(2)
        .map(|w| (w[1] - w[0]).max(2) as usize - 2)
        .sum();
    let counts = Counts {
        vertices: me.verts_num(),
        edges: me.edges_num(),
        faces: me.faces_num(),
        triangles,
    };
    (counts, bounds_of(me.positions().iter().copied()))
}

/// Counts and local bounds of an edit mesh.
pub(crate) fn bmesh_stats(bm: &BMesh) -> (Counts, Option<[[f32; 3]; 2]>) {
    let triangles = bm
        .faces
        .iter()
        .map(|(_, f)| (f.len.max(2) - 2) as usize)
        .sum();
    let counts = Counts {
        vertices: bm.verts_num(),
        edges: bm.edges_num(),
        faces: bm.faces_num(),
        triangles,
    };
    (counts, bounds_of(bm.verts.iter().map(|(_, v)| v.co)))
}

/// The axis-aligned box of `points` (`None` when empty).
fn bounds_of(points: impl Iterator<Item = [f32; 3]>) -> Option<[[f32; 3]; 2]> {
    let mut b: Option<[[f32; 3]; 2]> = None;
    for p in points {
        let bb = b.get_or_insert([p, p]);
        for i in 0..3 {
            bb[0][i] = bb[0][i].min(p[i]);
            bb[1][i] = bb[1][i].max(p[i]);
        }
    }
    b
}

/// The world-space box of the local box `b` under `m` (its 8 corners transformed).
pub(crate) fn world_bounds(b: [[f32; 3]; 2], m: &Mat4) -> [[f32; 3]; 2] {
    let corners = (0..8).map(|i| {
        let c = Vec3::new(b[i & 1][0], b[(i >> 1) & 1][1], b[(i >> 2) & 1][2]);
        m.transform_point3(c).to_array()
    });
    bounds_of(corners).expect("eight corners")
}

fn vec3(v: [f32; 3]) -> Value {
    Value::FloatArray {
        data: SmallVec::from_slice(&v),
        shape: ArrayShape::Fixed(3),
        tag: MathTag::None,
    }
}

/// The slot materials of an object (`None` for empty slots).
fn materials_of(main: &Main, ob: IdRef<ObjectId>) -> Vec<Option<IdRef<MaterialId>>> {
    main.objects
        .get(ob)
        .map(|o| slots::slot_materials(main, o))
        .unwrap_or_default()
}

fn material_name(main: &Main, m: Option<IdRef<MaterialId>>) -> SmolStr {
    m.and_then(|m| main.materials.get(m))
        .map(|m| m.header.name.clone())
        .unwrap_or_default()
}

#[operator(
    poll = poll::ALWAYS,
    tier = T0,
    notes = [
        "frap extension: answers in result (scene, mode, active_object, objects[], totals, materials) and changes nothing",
        "objects: ALL (every object of the view layer), VISIBLE, SELECTED; evaluated: counts and bounds with modifiers applied",
    ]
)]
fn exec(cx: &mut OpCtx, p: frap::SceneSummary) -> OpResult {
    struct Row {
        ob: IdRef<ObjectId>,
        select: bool,
        visible: bool,
    }
    let (rows, scene_name, frame, mode, active) = {
        let c = cx.ctx();
        let vis = c.base_visibility();
        let rows: Vec<Row> = c
            .view_layer()
            .bases
            .iter()
            .map(|b| {
                let v = vis.get(&b.object).copied().unwrap_or_default();
                Row {
                    ob: b.object,
                    select: b.selected(),
                    visible: v.visible,
                }
            })
            .filter(|r| match p.objects.value {
                Objects::All => true,
                Objects::Visible => r.visible,
                Objects::Selected => r.visible && r.select,
            })
            .collect();
        let s = c.scene();
        (
            rows,
            s.header.name.clone(),
            s.frame_current,
            c.mode().as_str(),
            c.active_object(),
        )
    };
    let mut totals = Counts::default();
    let mut used: Vec<SmolStr> = Vec::new();
    let mut objects: Vec<IndexMap<SmolStr, Value>> = Vec::with_capacity(rows.len());
    for row in rows {
        let Some(o) = cx.main().objects.get(row.ob).cloned() else {
            continue;
        };
        let mut m: IndexMap<SmolStr, Value> = IndexMap::new();
        m.insert("name".into(), o.header.name.clone().into());
        m.insert("type".into(), o.object_type().into());
        m.insert("data".into(), Value::Id(o.data));
        m.insert("parent".into(), o.parent.into());
        m.insert("mode".into(), o.mode.as_str().into());
        m.insert("select".into(), row.select.into());
        m.insert("visible".into(), row.visible.into());
        m.insert("location".into(), vec3(o.location));
        m.insert("rotation_euler".into(), vec3(o.rotation_euler));
        m.insert("scale".into(), vec3(o.scale));
        let mats: Vec<Value> = materials_of(cx.main(), row.ob)
            .into_iter()
            .map(|mat| {
                let n = material_name(cx.main(), mat);
                if mat.is_some() && !used.contains(&n) {
                    used.push(n.clone());
                }
                Value::Str(n)
            })
            .collect();
        m.insert("materials".into(), Value::Tuple(mats));
        let mods: Vec<IndexMap<SmolStr, Value>> = o
            .modifiers
            .iter()
            .map(|md| {
                let mut x = IndexMap::new();
                x.insert(SmolStr::new("name"), md.name.clone().into());
                x.insert("type".into(), md.settings.type_identifier().into());
                x.insert("show_viewport".into(), md.show_viewport.into());
                x
            })
            .collect();
        m.insert("modifiers".into(), Value::Structs(mods));
        if let Some(me) = o.data.and_then(|d| d.typed::<MeshId>()) {
            let in_edit = o.mode == ObjectMode::Edit;
            let edit_stats = if in_edit {
                let (_, edit) = cx.split_edit();
                edit.meshes.get(&me.key()).map(|em| bmesh_stats(&em.bm))
            } else {
                None
            };
            let ev = if p.evaluated.value || edit_stats.is_some() {
                Some(cx.evaluated(row.ob)?)
            } else {
                None
            };
            let (counts, local) = match (edit_stats, &ev) {
                (Some(s), _) => s,
                (None, Some(e)) if p.evaluated.value => match &e.mesh {
                    Some(mesh) => (mesh_stats(mesh).0, e.bounds),
                    None => (Counts::default(), e.bounds),
                },
                _ => cx.main().meshes.get(me).map(mesh_stats).unwrap_or_default(),
            };
            let world = match &ev {
                Some(e) => e.matrix_world,
                None => cx.evaluated(row.ob)?.matrix_world,
            };
            m.insert("vertices".into(), counts.vertices.into());
            m.insert("edges".into(), counts.edges.into());
            m.insert("faces".into(), counts.faces.into());
            m.insert("triangles".into(), counts.triangles.into());
            let wb = local.map(|b| world_bounds(b, &world));
            m.insert("bounds_min".into(), wb.map_or(Value::None, |b| vec3(b[0])));
            m.insert("bounds_max".into(), wb.map_or(Value::None, |b| vec3(b[1])));
            totals.add(counts);
        }
        objects.push(m);
    }
    cx.result("scene", scene_name);
    cx.result("frame_current", frame);
    cx.result("mode", mode);
    cx.result("active_object", Value::Id(active.map(|a| a.any())));
    cx.result("objects", Value::Structs(objects));
    cx.result("vertices", totals.vertices);
    cx.result("edges", totals.edges);
    cx.result("faces", totals.faces);
    cx.result("triangles", totals.triangles);
    cx.result(
        "materials",
        Value::Tuple(used.into_iter().map(Value::Str).collect()),
    );
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    use super::*;
    use frap_bmesh::FromMeshParams;
    use frap_mesh::primitives::cube::{Cube, cube_mesh};

    #[test]
    fn cube_counts_and_bounds() {
        let me = cube_mesh(&Cube::default());
        let (c, b) = mesh_stats(&me);
        assert_eq!(
            c,
            Counts {
                vertices: 8,
                edges: 12,
                faces: 6,
                triangles: 12
            }
        );
        assert_eq!(b, Some([[-1.0; 3], [1.0; 3]]));
        let bm = BMesh::from_mesh(&me, &FromMeshParams::default());
        assert_eq!(bmesh_stats(&bm), (c, b));
    }

    #[test]
    fn world_bounds_transform_every_corner() {
        let m = Mat4::from_scale_rotation_translation(
            Vec3::new(2.0, 1.0, 1.0),
            glam::Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),
            Vec3::new(0.0, 0.0, 3.0),
        );
        let b = world_bounds([[-1.0; 3], [1.0; 3]], &m);
        for (got, want) in b.iter().flatten().zip([-1.0, -2.0, 2.0, 1.0, 2.0, 4.0]) {
            assert!((got - want).abs() < 1e-5, "{b:?}");
        }
    }

    #[test]
    fn empty_bounds_are_none() {
        assert_eq!(bounds_of(std::iter::empty()), None);
    }
}
