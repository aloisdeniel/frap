// SPDX-License-Identifier: GPL-3.0-or-later
//! `Mesh` ⇄ `BMesh` (`BM_mesh_bm_from_me` / `BM_mesh_bm_to_me`, `bmesh/intern/
//! bmesh_mesh_convert.cc`; ARCHITECTURE §6.2, [MESH §2.5]). CONTRACT frozen by M2 W0,
//! implemented by WP 2.1.
//!
//! Ported from Blender 5.2.4. `from_mesh` keeps element order (slot = index), reads selection
//! from `.select_*` (selecting an edge or face selects its vertices and edges, as
//! `BM_edge/face_select_set` do; hidden elements are never selected), hiding from `.hide_*`,
//! flags from `sharp_face`/`sharp_edge`/`uv_seam` (read with Blender's type and domain
//! conversion), UV selection from `.uv_select_*` (only when all three exist), the material
//! from `material_index`, and every other attribute into a custom-data layer (layers sorted
//! by type, as `CustomData` keeps them). A `.pn.<uv>` pin layer is added for every UV map that
//! lacks one, and vertex groups become the vertex domain's `deform` column.
//!
//! `to_mesh` writes in iteration order. Attributes come out in `BM_mesh_bm_to_me`'s order:
//! the generic layers by domain (point, edge, face, corner) in layer order (pin layers without
//! a pinned corner are skipped), then `.select_vert`, `.hide_vert`, `.select_edge`,
//! `sharp_edge`, `uv_seam`, `.hide_edge`, `.select_poly`, `.hide_poly`, `sharp_face`,
//! `.uv_select_vert`, `.uv_select_edge`, `.uv_select_face`, `material_index` (each only when
//! some element needs it; the UV selection ones whenever a UV map exists), then `position`,
//! `.edge_verts`, `.corner_vert`, `.corner_edge`. Flags that are all false are therefore never
//! written; `to_mesh` is the **only** caller of `Mesh::remove_all_false_flags`.
//!
//! Not converted (the frap `Mesh` has no place for them yet): the active face index and the
//! selection history (`Mesh.act_face`, `mselect`; edit mode keeps them on the `BMesh`), and
//! shape keys (`frap-mesh` has no key blocks before M6: callers fill
//! [`BMesh::shape_layers`] themselves).

use std::sync::Arc;

use frap_mesh::{
    AttrData, Attribute, AttributeSet, Domain, Mesh, SELECT_EDGE, SELECT_POLY, SELECT_VERT,
    SHARP_EDGE, SHARP_FACE, UV_SEAM, UV_SELECT_EDGE, UV_SELECT_FACE, UV_SELECT_VERT,
};
use smol_str::SmolStr;

use crate::customdata::{CdLayer, CustomData};
use crate::types::{BMesh, CreateFlag, EdgeId, ElemFlag, ElemTypes, FaceId, LoopId, VertId};

/// `BMeshFromMeshParams`.
#[derive(Clone, Debug, PartialEq)]
pub struct FromMeshParams {
    /// `calc_face_normal`.
    pub calc_face_normal: bool,
    /// `calc_vert_normal`.
    pub calc_vert_normal: bool,
    /// `use_shapekey`: load the key blocks as shape layers.
    pub use_shapekey: bool,
    /// `active_shapekey` (1-based, 0 = basis).
    pub active_shapekey: i32,
}

impl Default for FromMeshParams {
    /// Edit-mode entry (`EDBM_mesh_make`): normals computed, shape keys loaded.
    fn default() -> Self {
        FromMeshParams {
            calc_face_normal: true,
            calc_vert_normal: true,
            use_shapekey: true,
            active_shapekey: 0,
        }
    }
}

/// `BMeshToMeshParams`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ToMeshParams {
    /// `calc_object_remap` (unused by frap: no hooks/parent vertices).
    pub calc_object_remap: bool,
    /// `update_shapekey_indices`.
    pub update_shapekey_indices: bool,
}

/// `BM_attribute_stored_in_bmesh_builtin`: attributes stored as BMesh topology or flags.
pub fn stored_in_bmesh_builtin(name: &str) -> bool {
    matches!(
        name,
        ".edge_verts"
            | ".corner_vert"
            | ".corner_edge"
            | ".hide_vert"
            | ".hide_edge"
            | ".hide_poly"
            | ".select_vert"
            | ".select_edge"
            | ".select_poly"
            | ".uv_select_vert"
            | ".uv_select_edge"
            | ".uv_select_face"
            | "position"
            | "uv_seam"
            | "material_index"
            | "sharp_face"
            | "sharp_edge"
    )
}

/// `BKE_uv_map_pin_name_get`: the pin layer of a UV map.
pub fn uv_pin_name(uv: &str) -> SmolStr {
    SmolStr::from(format!(".pn.{uv}"))
}

fn layers_of(me: &Mesh, domain: Domain) -> CustomData {
    let mut cd = CustomData::default();
    for a in me.attributes().iter() {
        if a.domain != domain || stored_in_bmesh_builtin(&a.name) {
            continue;
        }
        cd.add_layer(CdLayer {
            name: a.name.clone(),
            data: a.data.clone(),
        });
    }
    cd
}

fn bool_col(v: &[bool]) -> AttrData {
    AttrData::Bool(Arc::new(v.to_vec()))
}

impl BMesh {
    /// `BM_mesh_bm_from_me` into a new BMesh (`BM_mesh_create` with
    /// `BMALLOC_TEMPLATE_FROM_ME`).
    pub fn from_mesh(me: &Mesh, params: &FromMeshParams) -> BMesh {
        let nv = me.verts_num();
        let ne = me.edges_num();
        let nf = me.faces_num();
        let nc = me.corners_num();
        let mut bm = BMesh::with_alloc_size([nv as u32, ne as u32, nc as u32, nf as u32]);

        bm.vdata = layers_of(me, Domain::Point);
        bm.edata = layers_of(me, Domain::Edge);
        bm.pdata = layers_of(me, Domain::Face);
        bm.ldata = layers_of(me, Domain::Corner);
        // Temporary pin layers for UV maps without one.
        for uv in bm.ldata.float2_names() {
            let pin = uv_pin_name(&uv);
            if bm.ldata.find(&pin).is_none() {
                bm.ldata.add_layer(CdLayer {
                    name: pin,
                    data: AttrData::Bool(Arc::new(vec![false; nc])),
                });
            }
        }
        if let Some(dv) = me.deform_verts() {
            bm.vdata.deform = Some(dv.to_vec());
        }
        let uvs = bm.ldata.float2_names();
        let pick = |want: &Option<SmolStr>| -> Option<SmolStr> {
            want.as_ref()
                .filter(|n| uvs.contains(n))
                .cloned()
                .or_else(|| uvs.first().cloned())
        };
        bm.ldata.uv_active = pick(&me.uv_active);
        bm.ldata.uv_render = pick(&me.uv_active_render);

        if nv == 0 {
            return bm;
        }

        let vert_normals = if params.calc_vert_normal {
            Some(me.vert_normals())
        } else {
            None
        };
        let get = |name: &str, d: Domain| me.lookup_bool(name, d);
        let select_vert = get(SELECT_VERT, Domain::Point);
        let select_edge = get(SELECT_EDGE, Domain::Edge);
        let select_poly = get(SELECT_POLY, Domain::Face);
        let hide_vert = get(".hide_vert", Domain::Point);
        let hide_edge = get(".hide_edge", Domain::Edge);
        let hide_poly = get(".hide_poly", Domain::Face);
        let sharp_faces = get(SHARP_FACE, Domain::Face);
        let sharp_edges = get(SHARP_EDGE, Domain::Edge);
        let uv_seams = get(UV_SEAM, Domain::Edge);
        let uv_select_vert = get(UV_SELECT_VERT, Domain::Corner);
        let uv_select_edge = get(UV_SELECT_EDGE, Domain::Corner);
        let uv_select_face = get(UV_SELECT_FACE, Domain::Face);
        let need_uv_select =
            uv_select_vert.is_some() && uv_select_edge.is_some() && uv_select_face.is_some();
        let material_indices: Option<&[i32]> = match me.attributes().get("material_index") {
            Some(Attribute {
                domain: Domain::Face,
                data: AttrData::Int(v),
                ..
            }) => Some(v.as_slice()),
            _ => None,
        };
        let at = |v: &Option<std::borrow::Cow<'_, [bool]>>, i: usize| {
            v.as_ref()
                .is_some_and(|x| x.get(i).copied().unwrap_or(false))
        };

        let positions = me.positions();
        let mut vtable = Vec::with_capacity(nv);
        for (i, &co) in positions.iter().enumerate() {
            frap_math::budget::check();
            let v = bm.vert_create(co, None, CreateFlag::SKIP_CD);
            vtable.push(v);
            if at(&hide_vert, i) {
                bm.vert_mut(v).flag.insert(ElemFlag::HIDDEN);
            }
            if at(&select_vert, i) {
                bm.vert_select_set(v, true);
            }
            if let Some(n) = &vert_normals {
                bm.vert_mut(v).no = n[i];
            }
        }
        let mut etable = Vec::with_capacity(ne);
        for (i, ed) in me.edges().iter().enumerate() {
            frap_math::budget::check();
            let e = bm.edge_create(
                vtable[ed[0] as usize],
                vtable[ed[1] as usize],
                None,
                CreateFlag::SKIP_CD,
            );
            etable.push(e);
            bm.edge_mut(e).flag = ElemFlag::empty();
            if at(&uv_seams, i) {
                bm.edge_mut(e).flag.insert(ElemFlag::SEAM);
            }
            if at(&hide_edge, i) {
                bm.edge_mut(e).flag.insert(ElemFlag::HIDDEN);
            }
            if at(&select_edge, i) {
                bm.edge_select_set(e, true);
            }
            if !at(&sharp_edges, i) {
                bm.edge_mut(e).flag.insert(ElemFlag::SMOOTH);
            }
        }
        let corner_verts = me.corner_verts();
        let corner_edges = me.corner_edges();
        // Mesh face / corner index of every created face / loop slot (identity unless a face
        // was skipped).
        let mut face_src: Vec<usize> = Vec::with_capacity(nf);
        let mut loop_src: Vec<usize> = Vec::with_capacity(nc);
        for i in 0..nf {
            frap_math::budget::check();
            let r = me.face(i);
            let fv: Vec<VertId> = corner_verts[r.clone()]
                .iter()
                .map(|&v| vtable[v as usize])
                .collect();
            let fe: Vec<EdgeId> = corner_edges[r.clone()]
                .iter()
                .map(|&e| etable[e as usize])
                .collect();
            let Some(f) = bm.face_create(&fv, &fe, None, CreateFlag::SKIP_CD) else {
                continue;
            };
            face_src.push(i);
            if !at(&sharp_faces, i) {
                bm.face_mut(f).flag.insert(ElemFlag::SMOOTH);
            }
            if at(&hide_poly, i) {
                bm.face_mut(f).flag.insert(ElemFlag::HIDDEN);
            }
            if at(&select_poly, i) {
                bm.face_select_set(f, true);
            }
            bm.face_mut(f).mat_nr = material_indices
                .and_then(|m| m.get(i))
                .map_or(0, |&m| m as i16);
            for (j, l) in bm.face_loops(f).into_iter().enumerate() {
                let c = r.start + j;
                loop_src.push(c);
                if need_uv_select {
                    if at(&uv_select_vert, c) {
                        bm.lp_mut(l).flag.insert(ElemFlag::SELECT_UV);
                    }
                    if at(&uv_select_edge, c) {
                        bm.lp_mut(l).flag.insert(ElemFlag::SELECT_UV_EDGE);
                    }
                }
            }
            if need_uv_select && at(&uv_select_face, i) {
                bm.face_mut(f).flag.insert(ElemFlag::SELECT_UV);
            }
            if params.calc_face_normal {
                bm.face_normal_update(f);
            }
        }
        // Custom data was loaded by slot = index; re-gather when a face was skipped.
        if face_src.len() != nf {
            regather(&mut bm.pdata, &face_src);
        }
        if loop_src.len() != nc || loop_src.iter().enumerate().any(|(i, &c)| i != c) {
            regather(&mut bm.ldata, &loop_src);
        }
        bm.index.dirty = ElemTypes::all();
        bm.elem_index_ensure(ElemTypes::all());
        bm.select_history.clear();
        bm.act_face = None;
        bm
    }

    // Ported from blender/source/blender/editors/mesh/editmesh_utils.cc
    // (EDBM_mesh_make_from_mesh).
    /// `EDBM_mesh_make`: the edit-mode BMesh of `me` (`from_mesh` with the edit-mode
    /// parameters), with `select_mode` (`tool_settings.mesh_select_mode`) set and the
    /// selection flushed for it (`EDBM_selectmode_flush`), as entering Edit Mode does.
    pub fn edit_mesh_make(me: &Mesh, select_mode: crate::types::SelectMode) -> BMesh {
        let mut bm = BMesh::from_mesh(me, &FromMeshParams::default());
        bm.select_mode = select_mode;
        bm.select_flush_mode();
        bm
    }

    /// `BM_mesh_bm_to_me`: the mesh of this BMesh; non-geometry data (name, materials, active
    /// layers, texture space, shape-key pointer, vertex-group names) comes from `template`.
    pub fn to_mesh(&self, template: &Mesh, params: &ToMeshParams) -> Mesh {
        let _ = params;
        let verts: Vec<VertId> = self.vert_ids();
        let edges: Vec<EdgeId> = self.edge_ids();
        let faces: Vec<FaceId> = self.face_ids();
        let mut vidx = vec![u32::MAX; self.verts.capacity()];
        for (i, v) in verts.iter().enumerate() {
            vidx[v.slot()] = i as u32;
        }
        let mut eidx = vec![u32::MAX; self.edges.capacity()];
        for (i, e) in edges.iter().enumerate() {
            eidx[e.slot()] = i as u32;
        }
        let mut loops: Vec<LoopId> = Vec::with_capacity(self.loops_num());
        let mut offsets = Vec::with_capacity(faces.len() + 1);
        offsets.push(0i32);
        for &f in &faces {
            frap_math::budget::check();
            loops.extend(self.face_loops(f));
            offsets.push(loops.len() as i32);
        }
        let vslots: Vec<usize> = verts.iter().map(|v| v.slot()).collect();
        let eslots: Vec<usize> = edges.iter().map(|e| e.slot()).collect();
        let fslots: Vec<usize> = faces.iter().map(|f| f.slot()).collect();
        let lslots: Vec<usize> = loops.iter().map(|l| l.slot()).collect();

        let vf = |f: ElemFlag| verts.iter().any(|&v| self.vert(v).flag.intersects(f));
        let ef = |f: ElemFlag| edges.iter().any(|&e| self.edge(e).flag.intersects(f));
        let ff = |f: ElemFlag| faces.iter().any(|&x| self.face(x).flag.intersects(f));
        let need_select_vert = vf(ElemFlag::SELECT);
        let need_hide_vert = vf(ElemFlag::HIDDEN);
        let need_select_edge = ef(ElemFlag::SELECT);
        let need_hide_edge = ef(ElemFlag::HIDDEN);
        let need_uv_seams = ef(ElemFlag::SEAM);
        let need_sharp_edge = edges
            .iter()
            .any(|&e| !self.edge(e).flag.contains(ElemFlag::SMOOTH));
        let need_select_poly = ff(ElemFlag::SELECT);
        let need_hide_poly = ff(ElemFlag::HIDDEN);
        let need_sharp_face = faces
            .iter()
            .any(|&f| !self.face(f).flag.contains(ElemFlag::SMOOTH));
        let need_material_index = faces.iter().any(|&f| self.face(f).mat_nr != 0);
        let uv_names = self.ldata.float2_names();
        let need_uv_select = !uv_names.is_empty();

        let mut attrs = AttributeSet::default();
        // 1. Generic layers by domain, in layer order.
        let generic = |attrs: &mut AttributeSet, cd: &CustomData, slots: &[usize], d: Domain| {
            for l in &cd.layers {
                if d == Domain::Corner && is_unpinned_pin(l, slots, &uv_names) {
                    continue;
                }
                attrs.insert(Attribute {
                    name: l.name.clone(),
                    domain: d,
                    data: l.data.gather_with_default(slots, slots.len()),
                });
            }
        };
        generic(&mut attrs, &self.vdata, &vslots, Domain::Point);
        generic(&mut attrs, &self.edata, &eslots, Domain::Edge);
        generic(&mut attrs, &self.pdata, &fslots, Domain::Face);
        generic(&mut attrs, &self.ldata, &lslots, Domain::Corner);
        // 2. Flag attributes.
        let mut add_bool = |name: &str, d: Domain, v: Vec<bool>| {
            attrs.insert(Attribute {
                name: name.into(),
                domain: d,
                data: bool_col(&v),
            });
        };
        let vflag = |f: ElemFlag| -> Vec<bool> {
            verts
                .iter()
                .map(|&v| self.vert(v).flag.contains(f))
                .collect()
        };
        let eflag = |f: ElemFlag| -> Vec<bool> {
            edges
                .iter()
                .map(|&e| self.edge(e).flag.contains(f))
                .collect()
        };
        let fflag = |f: ElemFlag| -> Vec<bool> {
            faces
                .iter()
                .map(|&x| self.face(x).flag.contains(f))
                .collect()
        };
        if need_select_vert {
            add_bool(SELECT_VERT, Domain::Point, vflag(ElemFlag::SELECT));
        }
        if need_hide_vert {
            add_bool(".hide_vert", Domain::Point, vflag(ElemFlag::HIDDEN));
        }
        if need_select_edge {
            add_bool(SELECT_EDGE, Domain::Edge, eflag(ElemFlag::SELECT));
        }
        if need_sharp_edge {
            let v = eflag(ElemFlag::SMOOTH).into_iter().map(|s| !s).collect();
            add_bool(SHARP_EDGE, Domain::Edge, v);
        }
        if need_uv_seams {
            add_bool(UV_SEAM, Domain::Edge, eflag(ElemFlag::SEAM));
        }
        if need_hide_edge {
            add_bool(".hide_edge", Domain::Edge, eflag(ElemFlag::HIDDEN));
        }
        if need_select_poly {
            add_bool(SELECT_POLY, Domain::Face, fflag(ElemFlag::SELECT));
        }
        if need_hide_poly {
            add_bool(".hide_poly", Domain::Face, fflag(ElemFlag::HIDDEN));
        }
        if need_sharp_face {
            let v = fflag(ElemFlag::SMOOTH).into_iter().map(|s| !s).collect();
            add_bool(SHARP_FACE, Domain::Face, v);
        }
        if need_uv_select {
            let lv = loops
                .iter()
                .map(|&l| self.lp(l).flag.contains(ElemFlag::SELECT_UV))
                .collect();
            add_bool(UV_SELECT_VERT, Domain::Corner, lv);
            let le = loops
                .iter()
                .map(|&l| self.lp(l).flag.contains(ElemFlag::SELECT_UV_EDGE))
                .collect();
            add_bool(UV_SELECT_EDGE, Domain::Corner, le);
            add_bool(UV_SELECT_FACE, Domain::Face, fflag(ElemFlag::SELECT_UV));
        }
        if need_material_index {
            attrs.insert(Attribute {
                name: "material_index".into(),
                domain: Domain::Face,
                data: AttrData::Int(Arc::new(
                    faces.iter().map(|&f| self.face(f).mat_nr as i32).collect(),
                )),
            });
        }
        // 3. Positions and topology.
        let positions: Vec<[f32; 3]> = verts.iter().map(|&v| self.vert(v).co).collect();
        let edge_verts: Vec<[i32; 2]> = edges
            .iter()
            .map(|&e| {
                let [a, b] = self.edge(e).v;
                [vidx[a.slot()] as i32, vidx[b.slot()] as i32]
            })
            .collect();
        let corner_verts: Vec<i32> = loops
            .iter()
            .map(|&l| vidx[self.lp(l).v.slot()] as i32)
            .collect();
        let corner_edges: Vec<i32> = loops
            .iter()
            .map(|&l| eidx[self.lp(l).e.slot()] as i32)
            .collect();
        attrs.insert(Attribute {
            name: "position".into(),
            domain: Domain::Point,
            data: AttrData::Float3(Arc::new(positions)),
        });
        attrs.insert(Attribute {
            name: ".edge_verts".into(),
            domain: Domain::Edge,
            data: AttrData::Int32x2(Arc::new(edge_verts.clone())),
        });
        attrs.insert(Attribute {
            name: ".corner_vert".into(),
            domain: Domain::Corner,
            data: AttrData::Int(Arc::new(corner_verts.clone())),
        });
        attrs.insert(Attribute {
            name: ".corner_edge".into(),
            domain: Domain::Corner,
            data: AttrData::Int(Arc::new(corner_edges)),
        });

        let mut me = template.clone();
        let keep = (
            me.vertex_group_names.clone(),
            me.uv_active.clone(),
            me.uv_active_render.clone(),
            me.color_active.clone(),
            me.color_default.clone(),
        );
        me.clear_geometry();
        (
            me.vertex_group_names,
            me.uv_active,
            me.uv_active_render,
            me.color_active,
            me.color_default,
        ) = keep;
        me.set_topology(offsets, corner_verts, edge_verts);
        *me.attributes_mut() = attrs;
        if let Some(dv) = &self.vdata.deform {
            let gathered: Vec<_> = vslots
                .iter()
                .map(|&s| dv.get(s).cloned().unwrap_or_default())
                .collect();
            *me.deform_verts_mut() = gathered;
        }
        if need_uv_select {
            if let Some(a) = &self.ldata.uv_active {
                me.uv_active = Some(a.clone());
            } else {
                me.uv_active = uv_names.first().cloned();
            }
            if let Some(r) = &self.ldata.uv_render {
                me.uv_active_render = Some(r.clone());
            } else {
                me.uv_active_render = uv_names.first().cloned();
            }
        }
        me.remove_all_false_flags();
        me
    }
}

/// A `.pn.<uv>` layer of an existing UV map with no pinned corner (`CD_FLAG_NOCOPY` in
/// `bm_face_loop_table_build`).
fn is_unpinned_pin(l: &CdLayer, slots: &[usize], uvs: &[SmolStr]) -> bool {
    let Some(uv) = l.name.strip_prefix(".pn.") else {
        return false;
    };
    if !uvs.iter().any(|u| u == uv) {
        return false;
    }
    match &l.data {
        AttrData::Bool(v) => !slots.iter().any(|&s| v.get(s).copied().unwrap_or(false)),
        _ => false,
    }
}

/// Re-orders the columns of `cd` (loaded by mesh index) so that slot `i` holds mesh element
/// `src[i]`.
fn regather(cd: &mut CustomData, src: &[usize]) {
    for l in &mut cd.layers {
        l.data = l.data.gather_with_default(src, src.len());
    }
}
