// SPDX-License-Identifier: GPL-3.0-or-later
//! BOOLEAN (`BooleanModifier`): the object's mesh combined with an operand object, or with the
//! mesh objects of a collection, through [`frap_boolean`].
//!
//! Ported from blender/source/blender/modifiers/intern/MOD_boolean.cc (`is_disabled`,
//! `foreach_ID_link`, `BMD_error_messages`, `get_material_remap_index_based`,
//! `get_material_remap_transfer`, `non_float_boolean_mesh`, `modify_mesh`) and the mesh side
//! of blender/source/blender/geometry/intern/mesh_boolean_manifold.cc and mesh_boolean.cc
//! (`meshgl_to_mesh`, `OutToInMaps::ensure_edge_map`, `copy_attribute_using_map`,
//! `interpolate_corner_attributes`, `set_material_from_map`, `get_intersecting_edges`, the
//! joined mesh of `join_meshes_with_transforms`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! Solvers: MANIFOLD at T2; EXACT at T3 (manifold-rust stands in for Blender's exact mesh
//! arrangement, see `frap_boolean::manifold`); FLOAT runs EXACT and reports a WARNING
//! (`Boolean: the FLOAT solver runs as EXACT …`) until M8 (WP 8.6). The FLOAT-only refusals
//! (`Cannot execute, intersect only available using exact solver`, the empty collection) are
//! Blender's and kept.
//!
//! Attributes follow Blender's mesh boolean: point, edge and face attributes are copied from
//! the input element each output element maps to (default value when none), corner attributes
//! (UV maps) too, and the corners at new vertices are interpolated in their input face with
//! mean-value weights. `material_index` is remapped per operand: INDEX by slot (the operand's
//! material found in the object's slots, else the same slot, else 0), TRANSFER by material
//! (the result's materials are the object's mesh materials followed by the operands' new
//! ones).

use std::borrow::Cow;
use std::sync::Arc;

use frap_boolean::{BoolError, BoolMesh, BoolOperation, BoolOptions, BooleanSolver, SolverKind};
use frap_mesh::{AttrData, Attribute, Domain, Mesh};
use frap_rna::{CollectionId, IdAny, IdRef, MaterialId, ObjectId, Report, ReportType};
use frap_scene::Main;
use frap_scene::generated::modifier_settings::BooleanSettings;
use frap_scene::id::slots;
use glam::Mat4;
use rustc_hash::FxHashMap;
use smallvec::SmallVec;

use crate::Object;
use crate::prelude::*;

/// `foreach_ID_link`: the operand object and the collection.
fn depends_on(s: &ModifierSettings) -> SmallVec<[IdAny; 2]> {
    let mut v = SmallVec::new();
    if let ModifierSettings::Boolean(b) = s {
        if b.operand_type == "OBJECT" {
            v.extend(b.object);
        } else {
            v.extend(b.collection);
        }
    }
    v
}

/// `is_disabled`: an OBJECT operand without object (a non-mesh object is skipped in `eval`),
/// or a COLLECTION operand without collection unless the solver is EXACT (which tolerates an
/// empty collection).
fn is_disabled(s: &ModifierSettings) -> Option<&'static str> {
    let ModifierSettings::Boolean(b) = s else {
        return None;
    };
    if b.operand_type == "OBJECT" {
        b.object.is_none().then_some("Boolean: no operand object")
    } else if b.collection.is_none() && b.solver != "EXACT" {
        Some("Boolean: no operand collection")
    } else {
        None
    }
}

#[modifier(
    BOOLEAN,
    kind = Nonconstructive,
    flags = ACCEPTS_MESH | SUPPORTS_EDITMODE,
    depends_on = depends_on,
    is_disabled = is_disabled,
    krate = crate
)]
fn eval(md: &Modifier, mesh: Mesh, cx: &mut dyn ModEvalCx) -> Result<Mesh, ModError> {
    let ModifierSettings::Boolean(s) = &md.settings else {
        return Ok(mesh);
    };
    let s = s.clone();
    if let Some(e) = error_messages(&s, cx.main()) {
        return Err(ModError::Error(e.into()));
    }
    if s.solver == "FLOAT" {
        cx.report(Report::new(
            ReportType::Warning,
            format!(
                "Modifier '{}': the FLOAT solver runs as EXACT in frap (Blender's BMesh \
                 intersect is not ported until M8)",
                md.name
            ),
        ));
    }
    non_float_boolean_mesh(&s, mesh, cx)
}

/// `BMD_error_messages`: the refusals that return the input mesh.
fn error_messages(s: &BooleanSettings, main: &Main) -> Option<&'static str> {
    if s.operand_type != "COLLECTION" {
        return None;
    }
    let use_exact = s.solver == "EXACT";
    let use_manifold = s.solver == "MANIFOLD";
    let mut err = None;
    if s.operation == "INTERSECT" && !(use_exact || use_manifold) {
        err = Some("Cannot execute, intersect only available using exact solver");
    }
    let col = s
        .collection
        .and_then(|c| c.typed::<CollectionId>())
        .and_then(|c| main.collections.get(c));
    if let Some(col) = col {
        if !use_exact && col.objects.is_empty() && col.children.is_empty() {
            err = Some("Cannot execute, non-exact solver and empty collection");
        }
        let any_non_mesh = collection_objects(main, s.collection)
            .iter()
            .any(|&o| main.objects.get(o).is_some_and(|o| o.object_type() != "MESH"));
        if any_non_mesh {
            err = Some("Cannot execute, the selected collection contains non mesh objects");
        }
    }
    err
}

/// `FOREACH_COLLECTION_OBJECT_RECURSIVE`: the collection's objects, then its children's, each
/// once.
pub fn collection_objects(main: &Main, col: Option<IdAny>) -> Vec<IdRef<ObjectId>> {
    fn walk(main: &Main, c: IdRef<CollectionId>, depth: usize, out: &mut Vec<IdRef<ObjectId>>) {
        let Some(col) = main.collections.get(c) else {
            return;
        };
        if depth > 256 {
            return;
        }
        for &o in &col.objects {
            if !out.contains(&o) {
                out.push(o);
            }
        }
        for &ch in &col.children {
            walk(main, ch, depth + 1, out);
        }
    }
    let mut out = Vec::new();
    if let Some(c) = col.and_then(|c| c.typed::<CollectionId>()) {
        walk(main, c, 0, &mut out);
    }
    out
}

/// `BKE_object_material_get(ob, i + 1)` for every slot.
fn object_materials(main: &Main, ob: &Object) -> Vec<Option<IdRef<MaterialId>>> {
    (0..slots::slot_count(main, ob))
        .map(|i| slots::slot(main, ob, i).1)
        .collect()
}

/// `get_material_remap_index_based` (`BKE_object_material_remap_calc(dest, src)`): the slot
/// of `dest` holding each of `src`'s materials, else the same slot when `dest` has it, else 0.
pub fn material_remap_index_based(
    dest: &[Option<IdRef<MaterialId>>],
    src: &[Option<IdRef<MaterialId>>],
) -> Vec<i32> {
    if src.is_empty() {
        return vec![0];
    }
    let mut map: FxHashMap<Option<IdRef<MaterialId>>, i32> = FxHashMap::default();
    for (i, m) in dest.iter().enumerate() {
        map.entry(*m).or_insert(i as i32);
    }
    let mut remap: Vec<i32> = (0..src.len())
        .map(|i| if i < dest.len() { i as i32 } else { 0 })
        .collect();
    for (i, m) in src.iter().enumerate() {
        if i < dest.len() && dest[i] == *m {
            continue;
        }
        if let Some(&d) = map.get(m) {
            remap[i] = d;
        }
    }
    remap
}

/// `non_float_boolean_mesh`.
fn non_float_boolean_mesh(
    s: &BooleanSettings,
    mesh: Mesh,
    cx: &mut dyn ModEvalCx,
) -> Result<Mesh, ModError> {
    let operand_object = s.operand_type == "OBJECT";
    let self_ref = cx.object_ref();
    let world = cx.object_matrix_world(self_ref);
    let world_to_object = world.inverse();
    let main_objects: Vec<IdRef<ObjectId>> = if operand_object {
        match s.object.and_then(|o| o.typed::<ObjectId>()) {
            Some(o) => vec![o],
            None => return Ok(mesh),
        }
    } else {
        collection_objects(cx.main(), s.collection)
    };
    let transfer = s.material_mode == "TRANSFER";
    let self_slots = object_materials(cx.main(), cx.object());

    // The result's materials for TRANSFER (`VectorSet<Material *>`).
    let mut materials: Vec<Option<IdRef<MaterialId>>> = Vec::new();
    if transfer {
        if mesh.materials.is_empty() {
            materials.push(None);
        } else {
            for m in &mesh.materials {
                if !materials.contains(m) {
                    materials.push(*m);
                }
            }
        }
    }

    let mut operands: Vec<(Arc<Mesh>, Mat4)> = Vec::new();
    let mut names: Vec<String> = vec![cx.object().header.name.to_string()];
    let mut remaps: Vec<Vec<i32>> = vec![Vec::new()];
    for ob_ref in main_objects {
        let (is_mesh, name) = match cx.main().objects.get(ob_ref) {
            Some(o) => (o.object_type() == "MESH", o.header.name.to_string()),
            None => continue,
        };
        if !is_mesh {
            if operand_object {
                // `is_disabled`: a non-mesh operand object disables the modifier.
                return Ok(mesh);
            }
            continue;
        }
        if !operand_object && ob_ref == self_ref {
            continue;
        }
        let ev = match cx.evaluated_mesh(ob_ref) {
            Ok(m) => m,
            Err(ModError::Dependency(_)) if !operand_object => continue,
            Err(_) => return Ok(mesh),
        };
        let m = cx.object_matrix_world(ob_ref);
        let ob_slots = cx
            .main()
            .objects
            .get(ob_ref)
            .map(|o| object_materials(cx.main(), o))
            .unwrap_or_default();
        let remap = if transfer {
            // `get_material_remap_transfer`.
            (0..ev.materials.len())
                .map(|i| match ob_slots.get(i).copied().flatten() {
                    Some(ma) => {
                        if let Some(p) = materials.iter().position(|x| *x == Some(ma)) {
                            p as i32
                        } else {
                            materials.push(Some(ma));
                            materials.len() as i32 - 1
                        }
                    }
                    None => -1,
                })
                .collect()
        } else {
            material_remap_index_based(&self_slots, &ob_slots)
        };
        operands.push((ev, world_to_object * m));
        names.push(name);
        remaps.push(remap);
    }

    let solver_kind = if s.solver == "MANIFOLD" {
        SolverKind::Manifold
    } else {
        SolverKind::Exact
    };
    let mut opts = BoolOptions::default();
    opts.operation = match s.operation.as_str() {
        "INTERSECT" => BoolOperation::Intersect,
        "UNION" => BoolOperation::Union,
        _ => BoolOperation::Difference,
    };
    opts.solver = solver_kind;
    opts.use_self = s.use_self;
    opts.hole_tolerant = s.use_hole_tolerant;
    opts.double_threshold = s.double_threshold;
    let mut inputs: Vec<BoolInput<'_>> = vec![BoolInput {
        mesh: &mesh,
        matrix: Mat4::IDENTITY,
    }];
    inputs.extend(operands.iter().map(|(m, t)| BoolInput {
        mesh: m,
        matrix: *t,
    }));
    match mesh_boolean(cx.boolean_solver(), &inputs, &remaps, &opts, false) {
        Ok(mut out) => {
            if transfer {
                out.mesh.materials = materials;
            }
            Ok(out.mesh)
        }
        Err(BoolError::NonManifold(bad)) if !bad.is_empty() => {
            let list: Vec<String> = bad
                .iter()
                .map(|&i| format!("'{}'", names[i as usize]))
                .collect();
            if solver_kind == SolverKind::Exact {
                // frap deviation: Blender's exact solver accepts open operands.
                return Err(ModError::Error(format!(
                    "frap: the EXACT solver needs closed operands; object(s) {} are open \
                     (Blender's exact mesh arrangement is not ported, WP 8.6)",
                    list.join(", ")
                )));
            }
            Err(ModError::Error(format!(
                "Cannot execute, object(s) {} have non-manifold geometry",
                list.join(", ")
            )))
        }
        Err(BoolError::NonManifold(_)) => Err(ModError::Error(
            "Cannot execute, non-manifold inputs".into(),
        )),
        Err(BoolError::Input(msg)) | Err(BoolError::Unavailable(msg)) => {
            Err(ModError::Error(msg))
        }
        // `ResultTooBig` sets no modifier error and returns no mesh: the input is kept.
        Err(BoolError::ResultTooBig) => Ok(mesh.clone()),
        Err(_) => Err(ModError::Error("Cannot execute, unknown error".into())),
    }
}

/// One input of [`mesh_boolean`]: a mesh and its matrix into the first input's space.
#[derive(Clone, Copy, Debug)]
pub struct BoolInput<'a> {
    /// The mesh.
    pub mesh: &'a Mesh,
    /// Matrix into the common space (identity for the first input).
    pub matrix: Mat4,
}

/// The result of [`mesh_boolean`].
#[derive(Clone, Debug)]
pub struct BoolOutput {
    /// The result mesh (attributes transferred, materials of the joined inputs).
    pub mesh: Mesh,
    /// `r_intersecting_edges`: edges between faces of different inputs (when asked for).
    pub intersecting_edges: Vec<usize>,
    /// The input (index into the inputs) of every output face.
    pub face_input: Vec<u32>,
}

/// Per-input element offsets of the joined mesh (`MeshOffsets`).
struct Offsets {
    vert: Vec<usize>,
    edge: Vec<usize>,
    face: Vec<usize>,
    corner: Vec<usize>,
}

/// `geometry::boolean::mesh_boolean` for the MANIFOLD/EXACT solvers on frap meshes: transforms
/// the inputs into the first one's space, runs `solver` and builds the result mesh as
/// `meshgl_to_mesh` does (edges by `mesh_calc_edges`, attributes through the out-to-in maps,
/// `material_index` through `material_remaps` when one is given per input).
pub fn mesh_boolean(
    solver: &dyn BooleanSolver,
    inputs: &[BoolInput<'_>],
    material_remaps: &[Vec<i32>],
    opts: &BoolOptions,
    want_intersecting_edges: bool,
) -> Result<BoolOutput, BoolError> {
    // The joined mesh's positions: transformed copies (`join_meshes_with_transforms`).
    let meshes: Vec<Cow<'_, Mesh>> = inputs
        .iter()
        .map(|i| {
            if i.matrix == Mat4::IDENTITY {
                Cow::Borrowed(i.mesh)
            } else {
                let mut m = i.mesh.clone();
                m.transform(&i.matrix);
                Cow::Owned(m)
            }
        })
        .collect();
    let tris: Vec<Arc<Vec<[u32; 3]>>> = meshes.iter().map(|m| m.looptris()).collect();
    let offsets_u32: Vec<Vec<u32>> = meshes
        .iter()
        .map(|m| m.face_offsets().iter().map(|&o| o as u32).collect())
        .collect();
    let corner_u32: Vec<Vec<u32>> = meshes
        .iter()
        .map(|m| m.corner_verts().iter().map(|&v| v as u32).collect())
        .collect();
    let bool_meshes: Vec<BoolMesh<'_>> = meshes
        .iter()
        .enumerate()
        .map(|(i, m)| {
            BoolMesh::new(m.positions(), &offsets_u32[i], &corner_u32[i]).with_corner_tris(&tris[i])
        })
        .collect();
    let r = solver.run(&bool_meshes, opts)?;

    let mut off = Offsets {
        vert: vec![0],
        edge: vec![0],
        face: vec![0],
        corner: vec![0],
    };
    for m in &meshes {
        off.vert.push(off.vert.last().unwrap() + m.verts_num());
        off.edge.push(off.edge.last().unwrap() + m.edges_num());
        off.face.push(off.face.last().unwrap() + m.faces_num());
        off.corner.push(off.corner.last().unwrap() + m.corners_num());
    }

    // The joined mesh's materials (`realize_instances`: every input's slots, an empty slot for
    // an input without any) and each input's slot map.
    let mut joined_materials: Vec<Option<IdRef<MaterialId>>> = Vec::new();
    for m in &meshes {
        if m.materials.is_empty() {
            if !joined_materials.contains(&None) {
                joined_materials.push(None);
            }
        } else {
            for ma in &m.materials {
                if !joined_materials.contains(ma) {
                    joined_materials.push(*ma);
                }
            }
        }
    }

    let base = inputs[0].mesh;
    let mut out = Mesh::new(base.header.name.clone());
    copy_parameters_for_eval(&mut out, base);
    out.materials = joined_materials.clone();
    let faces_num = r.face_offsets.len().saturating_sub(1);
    if r.positions.is_empty() || faces_num == 0 {
        return Ok(BoolOutput {
            mesh: out,
            intersecting_edges: Vec::new(),
            face_input: Vec::new(),
        });
    }

    // Geometry, attributes in Blender's creation order: `.corner_vert`, `position`, then
    // `mesh_calc_edges`' `.corner_edge` and `.edge_verts`.
    let corner_verts: Vec<i32> = r.corner_verts.iter().map(|&v| v as i32).collect();
    out.attributes_mut().insert(Attribute {
        name: ".corner_vert".into(),
        domain: Domain::Corner,
        data: AttrData::Int(Arc::new(corner_verts.clone())),
    });
    out.set_positions(r.positions.clone());
    out.set_topology(
        r.face_offsets.iter().map(|&o| o as i32).collect(),
        corner_verts,
        Vec::new(),
    );
    out.calc_edges(false, false);

    // Out-to-in maps in the joined index space.
    let face_map: Vec<usize> = r
        .face_src
        .iter()
        .map(|&(m, f)| off.face[m as usize] + f as usize)
        .collect();
    let vert_map: Vec<Option<usize>> = r
        .vert_src
        .iter()
        .map(|s| s.map(|(m, v)| off.vert[m as usize] + v as usize))
        .collect();
    let corner_map: Vec<Option<usize>> = r
        .corner_src
        .iter()
        .map(|s| s.map(|(m, c)| off.corner[m as usize] + c as usize))
        .collect();
    let joined = Joined::new(&meshes, &off);
    let edge_map = ensure_edge_map(&out, &joined, &off, &face_map, &vert_map, &corner_map);

    // Attributes of the joined mesh, first input first.
    let mut names: Vec<(smol_str::SmolStr, Domain)> = Vec::new();
    for m in &meshes {
        for a in m.attributes().iter() {
            if matches!(
                a.name.as_str(),
                "position" | ".edge_verts" | ".corner_vert" | ".corner_edge"
            ) {
                continue;
            }
            if !names.iter().any(|(n, _)| *n == a.name) {
                names.push((a.name.clone(), a.domain));
            }
        }
    }
    let has_material_index =
        names.iter().any(|(n, _)| n == "material_index") || joined_materials.len() > 1;
    for (name, domain) in names.iter().filter(|(_, d)| *d != Domain::Corner) {
        if name == "material_index" {
            continue;
        }
        let Some(src) = joined.column(&meshes, &off, name, *domain) else {
            continue;
        };
        let map: &[Option<usize>] = match domain {
            Domain::Point => &vert_map,
            Domain::Edge => &edge_map,
            _ => &face_map_opt(&face_map),
        };
        out.attributes_mut().insert(Attribute {
            name: name.clone(),
            domain: *domain,
            data: gather_opt(&src, map),
        });
    }
    if has_material_index {
        let mut mi = Vec::with_capacity(faces_num);
        for &(m, f) in &r.face_src {
            let m = m as usize;
            let orig = meshes[m]
                .material_indices()
                .map(|s| s[f as usize])
                .unwrap_or(0);
            let v = if material_remaps.is_empty() {
                // The joined mesh's index (`copy_attribute_using_map`).
                joined_material_index(&meshes[m], &joined_materials, orig)
            } else {
                // `set_material_from_map`.
                let map = material_remaps.get(m).map(Vec::as_slice).unwrap_or(&[]);
                if orig >= 0 && (orig as usize) < map.len() {
                    map[orig as usize]
                } else {
                    orig
                }
            };
            mi.push(v);
        }
        out.attributes_mut().insert(Attribute {
            name: "material_index".into(),
            domain: Domain::Face,
            data: AttrData::Int(Arc::new(mi)),
        });
    }
    // Vertex groups (`MDeformVert` through the vertex map, groups merged by name).
    transfer_vertex_groups(&mut out, &meshes, &off, &vert_map);
    interpolate_corner_attributes(&mut out, &meshes, &off, &joined, &names, &face_map, &corner_map);

    let face_input: Vec<u32> = r.face_src.iter().map(|s| s.0).collect();
    let intersecting_edges = if want_intersecting_edges {
        intersecting_edges(&out, &face_input)
    } else {
        Vec::new()
    };
    Ok(BoolOutput {
        mesh: out,
        intersecting_edges,
        face_input,
    })
}

/// The face map as an optional map (every output face has a source).
fn face_map_opt(face_map: &[usize]) -> Vec<Option<usize>> {
    face_map.iter().map(|&f| Some(f)).collect()
}

/// `get_mapped_material_index` of `realize_instances`: an input's slot in the joined slots.
fn joined_material_index(m: &Mesh, joined: &[Option<IdRef<MaterialId>>], orig: i32) -> i32 {
    let first = |ma: Option<IdRef<MaterialId>>| {
        joined.iter().position(|x| *x == ma).unwrap_or(0) as i32
    };
    if m.materials.is_empty() {
        return first(None);
    }
    if orig >= 0 && (orig as usize) < m.materials.len() {
        first(m.materials[orig as usize])
    } else {
        0
    }
}

/// `BKE_mesh_copy_parameters_for_eval`: the metadata the result keeps from the first input.
fn copy_parameters_for_eval(dst: &mut Mesh, src: &Mesh) {
    dst.materials = src.materials.clone();
    dst.vertex_group_names = src.vertex_group_names.clone();
    dst.uv_active = src.uv_active.clone();
    dst.uv_active_render = src.uv_active_render.clone();
    dst.color_active = src.color_active.clone();
    dst.color_default = src.color_default.clone();
    dst.texspace = src.texspace;
}

/// Joined-mesh topology (edges and corners in the concatenated index space).
struct Joined {
    positions: Vec<[f32; 3]>,
    edges: Vec<[usize; 2]>,
    face_offsets: Vec<usize>,
    corner_verts: Vec<usize>,
    corner_edges: Vec<usize>,
}

impl Joined {
    fn new(meshes: &[Cow<'_, Mesh>], off: &Offsets) -> Self {
        let mut j = Joined {
            positions: Vec::with_capacity(*off.vert.last().unwrap()),
            edges: Vec::with_capacity(*off.edge.last().unwrap()),
            face_offsets: vec![0],
            corner_verts: Vec::with_capacity(*off.corner.last().unwrap()),
            corner_edges: Vec::with_capacity(*off.corner.last().unwrap()),
        };
        for (i, m) in meshes.iter().enumerate() {
            let (vo, eo, co) = (off.vert[i], off.edge[i], off.corner[i]);
            j.positions.extend_from_slice(m.positions());
            j.edges
                .extend(m.edges().iter().map(|e| [e[0] as usize + vo, e[1] as usize + vo]));
            j.face_offsets
                .extend(m.face_offsets()[1..].iter().map(|&o| o as usize + co));
            j.corner_verts
                .extend(m.corner_verts().iter().map(|&v| v as usize + vo));
            j.corner_edges
                .extend(m.corner_edges().iter().map(|&e| e.max(0) as usize + eo));
        }
        j
    }

    fn face(&self, f: usize) -> std::ops::Range<usize> {
        self.face_offsets[f]..self.face_offsets[f + 1]
    }

    /// The joined column of attribute `name` on `domain`: every input's values (converted to
    /// the first input's type), defaults where an input lacks it.
    fn column(
        &self,
        meshes: &[Cow<'_, Mesh>],
        off: &Offsets,
        name: &str,
        domain: Domain,
    ) -> Option<AttrData> {
        let first = meshes
            .iter()
            .find_map(|m| m.attributes().get(name).filter(|a| a.domain == domain))?;
        let sizes = match domain {
            Domain::Point => &off.vert,
            Domain::Edge => &off.edge,
            Domain::Face => &off.face,
            Domain::Corner => &off.corner,
        };
        let total = *sizes.last().unwrap();
        // Indices into a concatenation of the per-input columns (missing ones are defaults).
        let mut parts: Vec<AttrData> = Vec::with_capacity(meshes.len());
        for (i, m) in meshes.iter().enumerate() {
            let n = sizes[i + 1] - sizes[i];
            let part = match m.attributes().get(name) {
                Some(a)
                    if a.domain == domain
                        && a.data.data_type() == first.data.data_type()
                        && a.data.len() == n =>
                {
                    a.data.clone()
                }
                _ => first.data.default_of_len(n),
            };
            parts.push(part);
        }
        Some(concat(&parts, total))
    }
}

/// Concatenates columns of one type.
fn concat(parts: &[AttrData], total: usize) -> AttrData {
    macro_rules! cat {
        ($variant:ident) => {{
            let mut v = Vec::with_capacity(total);
            for p in parts {
                if let AttrData::$variant(c) = p {
                    v.extend_from_slice(c);
                }
            }
            AttrData::$variant(Arc::new(v))
        }};
    }
    match &parts[0] {
        AttrData::Float(_) => cat!(Float),
        AttrData::Int(_) => cat!(Int),
        AttrData::Int8(_) => cat!(Int8),
        AttrData::Bool(_) => cat!(Bool),
        AttrData::Float2(_) => cat!(Float2),
        AttrData::Float3(_) => cat!(Float3),
        AttrData::Float4(_) => cat!(Float4),
        AttrData::FloatColor(_) => cat!(FloatColor),
        AttrData::ByteColor(_) => cat!(ByteColor),
        AttrData::Int16x2(_) => cat!(Int16x2),
        AttrData::Int32x2(_) => cat!(Int32x2),
        AttrData::Quaternion(_) => cat!(Quaternion),
        AttrData::Float4x4(_) => cat!(Float4x4),
        AttrData::String(_) => cat!(String),
        _ => parts[0].default_of_len(total),
    }
}

/// `copy_attribute_using_map`: `dst[i] = src[map[i]]`, the type's default where unmapped.
fn gather_opt(src: &AttrData, map: &[Option<usize>]) -> AttrData {
    let def = src.default_of_len(1);
    // Gather the mapped ones with index 0 placeholders, then reset the unmapped to defaults.
    let idx: Vec<usize> = map.iter().map(|m| m.unwrap_or(0)).collect();
    let mut out = if src.is_empty() {
        src.default_of_len(map.len())
    } else {
        src.gather_with_default(&idx, map.len())
    };
    for (i, m) in map.iter().enumerate() {
        if m.is_none() {
            set_from(&mut out, i, &def, 0);
        }
    }
    out
}

/// `dst[i] = src[j]` for columns of the same type.
fn set_from(dst: &mut AttrData, i: usize, src: &AttrData, j: usize) {
    macro_rules! set {
        ($($variant:ident),*) => {
            match (dst, src) {
                $((AttrData::$variant(d), AttrData::$variant(s)) => {
                    Arc::make_mut(d)[i] = s[j].clone();
                })*
                _ => {}
            }
        };
    }
    set!(
        Float, Int, Int8, Bool, Float2, Float3, Float4, FloatColor, ByteColor, Int16x2, Int32x2,
        Quaternion, Float4x4, String
    );
}

/// Ported from mesh_boolean_manifold.cc (`same_dir`).
fn same_dir(p1: [f32; 3], p2: [f32; 3], q1: [f32; 3], q2: [f32; 3]) -> bool {
    let p = [p1[0] - p2[0], p1[1] - p2[1], p1[2] - p2[2]];
    let q = [q1[0] - q2[0], q1[1] - q2[1], q1[2] - q2[2]];
    let len = |v: [f32; 3]| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    let pq = len(p) * len(q);
    if pq == 0.0 {
        return true;
    }
    let abs_cos = ((p[0] * q[0] + p[1] * q[1] + p[2] * q[2]) / pq).abs();
    (abs_cos - 1.0).abs() <= 1e-5
}

/// `OutToInMaps::ensure_edge_map`: the input edge each output edge runs along (both ends
/// matching, or one end and the same direction), faces and corners in order.
fn ensure_edge_map(
    out: &Mesh,
    joined: &Joined,
    off: &Offsets,
    face_map: &[usize],
    vert_map: &[Option<usize>],
    corner_map: &[Option<usize>],
) -> Vec<Option<usize>> {
    let edges_num = out.edges_num();
    let mut edge_map: Vec<Option<usize>> = vec![None; edges_num];
    let mut done = vec![false; edges_num];
    let out_edges = out.edges();
    let out_cv = out.corner_verts();
    let out_ce = out.corner_edges();
    let out_pos = out.positions();
    let vmap = |v: usize| vert_map[v];
    for (out_f, &in_f) in face_map.iter().enumerate() {
        let in_face = joined.face(in_f);
        let mesh_id = off.face[1..].partition_point(|&s| s <= in_f);
        let vrange = off.vert[mesh_id]..off.vert[mesh_id + 1];
        for out_c in out.face(out_f) {
            let Some(in_c) = corner_map[out_c] else {
                continue;
            };
            let out_e = out_ce[out_c] as usize;
            if done[out_e] {
                continue;
            }
            let out_v = out_cv[out_c] as usize;
            let in_e = joined.corner_edges[in_c];
            let in_v = joined.corner_verts[in_c];
            let mut oev = [out_edges[out_e][0] as usize, out_edges[out_e][1] as usize];
            if oev[0] != out_v {
                oev.swap(0, 1);
            }
            let mut iev = joined.edges[in_e];
            if iev[0] != in_v {
                iev.swap(0, 1);
            }
            let mut edge_rep = None;
            // Both ends match, or the far end is new (or another input's) and the edge runs
            // in the same direction.
            if vmap(oev[1]) == Some(iev[1])
                || (!vmap(oev[1]).is_some_and(|v| vrange.contains(&v))
                    && same_dir(
                        out_pos[oev[0]],
                        out_pos[oev[1]],
                        joined.positions[iev[0]],
                        joined.positions[iev[1]],
                    ))
            {
                edge_rep = Some(in_e);
            }
            if edge_rep.is_none() {
                // The output face may wind against the input face: try the previous edge.
                let in_c_prev = if in_c == in_face.start {
                    in_face.end - 1
                } else {
                    in_c - 1
                };
                let in_e_prev = joined.corner_edges[in_c_prev];
                let in_v_prev = joined.corner_verts[in_c_prev];
                let mut iev_prev = joined.edges[in_e_prev];
                if iev_prev[0] != in_v_prev {
                    iev_prev.swap(0, 1);
                }
                if vmap(oev[0]) == Some(iev_prev[1])
                    && (vmap(oev[1]) == Some(iev_prev[0])
                        || (vmap(oev[1]).is_none()
                            && same_dir(
                                out_pos[oev[0]],
                                out_pos[oev[1]],
                                joined.positions[iev_prev[0]],
                                joined.positions[iev_prev[1]],
                            )))
                {
                    edge_rep = Some(in_e_prev);
                }
            }
            if let Some(e) = edge_rep {
                edge_map[out_e] = Some(e);
                done[out_e] = true;
            }
        }
    }
    edge_map
}

/// Vertex groups: the result keeps the first input's names, then every other input's new
/// ones; each mapped vertex takes its source vertex's weights.
fn transfer_vertex_groups(
    out: &mut Mesh,
    meshes: &[Cow<'_, Mesh>],
    off: &Offsets,
    vert_map: &[Option<usize>],
) {
    if meshes.iter().all(|m| m.deform_verts().is_none()) {
        return;
    }
    let mut names = out.vertex_group_names.clone();
    let mut group_maps: Vec<Vec<u32>> = Vec::with_capacity(meshes.len());
    for m in meshes {
        let map = m
            .vertex_group_names
            .iter()
            .map(|n| match names.iter().position(|x| x == n) {
                Some(p) => p as u32,
                None => {
                    names.push(n.clone());
                    names.len() as u32 - 1
                }
            })
            .collect();
        group_maps.push(map);
    }
    out.vertex_group_names = names;
    let dverts = out.deform_verts_mut();
    for (v, src) in vert_map.iter().enumerate() {
        let Some(src) = *src else {
            continue;
        };
        let m = off.vert[1..].partition_point(|&s| s <= src);
        let Some(dv) = meshes[m].deform_verts() else {
            continue;
        };
        let local = src - off.vert[m];
        if let Some(d) = dv.get(local) {
            let mut w = d.clone();
            for (g, _) in &mut w.weights {
                if let Some(&ng) = group_maps[m].get(*g as usize) {
                    *g = ng;
                }
            }
            dverts[v] = w;
        }
    }
}

/// `interpolate_corner_attributes`: mapped corners copy their source corner; the corners of
/// a face with an unmapped corner at new vertices take the mean-value interpolation of the
/// input face's corners (`interp_weights_poly_v2` in the face's dominant-axis plane).
fn interpolate_corner_attributes(
    out: &mut Mesh,
    meshes: &[Cow<'_, Mesh>],
    off: &Offsets,
    joined: &Joined,
    names: &[(smol_str::SmolStr, Domain)],
    face_map: &[usize],
    corner_map: &[Option<usize>],
) {
    let mut srcs: Vec<(smol_str::SmolStr, AttrData)> = Vec::new();
    for (name, d) in names {
        if *d != Domain::Corner {
            continue;
        }
        if let Some(c) = joined.column(meshes, off, name, Domain::Corner) {
            srcs.push((name.clone(), c));
        }
    }
    if srcs.is_empty() {
        return;
    }
    let corners = out.corners_num();
    let mut dsts: Vec<AttrData> = srcs.iter().map(|(_, s)| s.default_of_len(corners)).collect();
    let out_cv: Vec<i32> = out.corner_verts().to_vec();
    let out_pos: Vec<[f32; 3]> = out.positions().to_vec();
    for (out_f, &in_f) in face_map.iter().enumerate() {
        let in_face = joined.face(in_f);
        let out_face = out.face(out_f);
        let needs = out_face.clone().any(|c| corner_map[c].is_none());
        if !needs {
            for c in out_face {
                let ic = corner_map[c].unwrap();
                for (k, (_, src)) in srcs.iter().enumerate() {
                    set_from(&mut dsts[k], c, src, ic);
                }
            }
            continue;
        }
        let in_verts: Vec<i32> = joined.corner_verts[in_face.clone()]
            .iter()
            .map(|&v| v as i32)
            .collect();
        let normal = frap_geom::normals::face_normal_calc(&joined.positions, &in_verts);
        let axis = frap_bmesh::ops::contextual_create::bli::axis_dominant_v3_to_m3(normal);
        let cos_2d: Vec<[f32; 2]> = in_verts
            .iter()
            .map(|&v| {
                frap_bmesh::ops::contextual_create::bli::mul_v2_m3v3(
                    &axis,
                    joined.positions[v as usize],
                )
            })
            .collect();
        let out_face_verts: Vec<i32> = out_cv[out_face.clone()].to_vec();
        let out_normal = frap_geom::normals::face_normal_calc(&out_pos, &out_face_verts);
        let flipped = normal[0] * out_normal[0] + normal[1] * out_normal[1]
            + normal[2] * out_normal[2]
            < 0.0;
        for c in out_face {
            if let Some(ic) = corner_map[c] {
                for (k, (_, src)) in srcs.iter().enumerate() {
                    set_from(&mut dsts[k], c, src, ic);
                }
                continue;
            }
            let co = frap_bmesh::ops::contextual_create::bli::mul_v2_m3v3(
                &axis,
                out_pos[out_cv[c] as usize],
            );
            let w = frap_bmesh::query::geom::interp_weights_poly_v2(&cos_2d, co);
            for (k, (name, src)) in srcs.iter().enumerate() {
                let flip = flipped && name == frap_mesh::CUSTOM_NORMAL;
                mix_into(&mut dsts[k], c, src, in_face.clone(), &w, flip);
            }
        }
    }
    for ((name, _), data) in srcs.into_iter().zip(dsts) {
        out.attributes_mut().insert(Attribute {
            name,
            domain: Domain::Corner,
            data,
        });
    }
}

/// `DefaultMixer<T>`: the weighted mean of `src[range]` into `dst[i]` (integers and booleans
/// accumulate in floating point and round; types without a mixer keep the default).
fn mix_into(
    dst: &mut AttrData,
    i: usize,
    src: &AttrData,
    range: std::ops::Range<usize>,
    w: &[f32],
    flip: bool,
) {
    let total: f32 = w.iter().sum();
    let inv = if total > 0.0 { 1.0 / total } else { 0.0 };
    macro_rules! floats {
        ($d:expr, $s:expr, $n:expr) => {{
            let mut acc = [0.0f32; $n];
            for (k, c) in range.clone().enumerate() {
                for j in 0..$n {
                    acc[j] += $s[c][j] * w[k];
                }
            }
            if total > 0.0 {
                for j in 0..$n {
                    acc[j] *= inv;
                }
            }
            acc
        }};
    }
    match (dst, src) {
        (AttrData::Float(d), AttrData::Float(s)) => {
            let mut acc = 0.0f32;
            for (k, c) in range.enumerate() {
                acc += s[c] * w[k];
            }
            Arc::make_mut(d)[i] = if total > 0.0 { acc * inv } else { 0.0 };
        }
        (AttrData::Float2(d), AttrData::Float2(s)) => {
            Arc::make_mut(d)[i] = floats!(d, s, 2);
        }
        (AttrData::Float3(d), AttrData::Float3(s)) => {
            let mut v = floats!(d, s, 3);
            if flip {
                v = [-v[0], -v[1], -v[2]];
            }
            Arc::make_mut(d)[i] = v;
        }
        (AttrData::Float4(d), AttrData::Float4(s)) => {
            Arc::make_mut(d)[i] = floats!(d, s, 4);
        }
        (AttrData::FloatColor(d), AttrData::FloatColor(s)) => {
            Arc::make_mut(d)[i] = floats!(d, s, 4);
        }
        (AttrData::ByteColor(d), AttrData::ByteColor(s)) => {
            let mut acc = [0.0f32; 4];
            for (k, c) in range.enumerate() {
                for (j, a) in acc.iter_mut().enumerate() {
                    *a += f32::from(s[c][j]) * w[k];
                }
            }
            Arc::make_mut(d)[i] = if total > 0.0 {
                acc.map(|a| (a * inv) as u8)
            } else {
                [0; 4]
            };
        }
        (AttrData::Int(d), AttrData::Int(s)) => {
            let mut acc = 0.0f64;
            for (k, c) in range.enumerate() {
                acc += f64::from(s[c]) * f64::from(w[k]);
            }
            Arc::make_mut(d)[i] = if total > 0.0 {
                (acc * f64::from(inv)).round() as i32
            } else {
                0
            };
        }
        (AttrData::Int8(d), AttrData::Int8(s)) => {
            let mut acc = 0.0f32;
            for (k, c) in range.enumerate() {
                acc += f32::from(s[c]) * w[k];
            }
            Arc::make_mut(d)[i] = if total > 0.0 {
                (acc * inv).round() as i8
            } else {
                0
            };
        }
        (AttrData::Bool(d), AttrData::Bool(s)) => {
            let mut acc = 0.0f32;
            for (k, c) in range.enumerate() {
                acc += if s[c] { w[k] } else { 0.0 };
            }
            Arc::make_mut(d)[i] = total > 0.0 && acc * inv >= 0.5;
        }
        (AttrData::Int16x2(d), AttrData::Int16x2(s)) => {
            let mut acc = [0.0f32; 2];
            for (k, c) in range.enumerate() {
                acc[0] += f32::from(s[c][0]) * w[k];
                acc[1] += f32::from(s[c][1]) * w[k];
            }
            Arc::make_mut(d)[i] = if total > 0.0 {
                [(acc[0] * inv).round() as i16, (acc[1] * inv).round() as i16]
            } else {
                [0; 2]
            };
        }
        (AttrData::Int32x2(d), AttrData::Int32x2(s)) => {
            let mut acc = [0.0f64; 2];
            for (k, c) in range.enumerate() {
                acc[0] += f64::from(s[c][0]) * f64::from(w[k]);
                acc[1] += f64::from(s[c][1]) * f64::from(w[k]);
            }
            Arc::make_mut(d)[i] = if total > 0.0 {
                [
                    (acc[0] * f64::from(inv)).round() as i32,
                    (acc[1] * f64::from(inv)).round() as i32,
                ]
            } else {
                [0; 2]
            };
        }
        _ => {}
    }
}

/// `get_intersecting_edges`: the edges whose two faces come from different inputs.
fn intersecting_edges(out: &Mesh, face_input: &[u32]) -> Vec<usize> {
    let mut first: Vec<Option<usize>> = vec![None; out.edges_num()];
    let mut r = Vec::new();
    let ce = out.corner_edges();
    for f in 0..out.faces_num() {
        for c in out.face(f) {
            let e = ce[c] as usize;
            match first[e] {
                None => first[e] = Some(f),
                Some(f2) => {
                    if face_input[f] != face_input[f2] {
                        r.push(e);
                    }
                }
            }
        }
    }
    r
}

/// Material indices of a mesh, if it has the attribute.
trait MaterialIndices {
    fn material_indices(&self) -> Option<&[i32]>;
}

impl MaterialIndices for Mesh {
    fn material_indices(&self) -> Option<&[i32]> {
        match self.attributes().get("material_index") {
            Some(Attribute {
                domain: Domain::Face,
                data: AttrData::Int(v),
                ..
            }) => Some(v),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
