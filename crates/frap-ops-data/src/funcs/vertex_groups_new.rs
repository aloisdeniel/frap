// SPDX-License-Identifier: GPL-3.0-or-later
//! `obj.vertex_groups.new(name='Group')` (`rna_Object_vgroup_new` →
//! `BKE_object_defgroup_add_name`, WP 6.3): appends a group with a unique name (`''` gives
//! `Group`) and makes it active. Also the vertex-group helpers of the `VertexGroups` and
//! `VertexGroup` functions of this directory (`vertex_groups_*`, `vertex_group_*`).
//!
//! Ported from blender/source/blender/makesrna/intern/rna_object.cc and
//! blender/source/blender/blenkernel/intern/object_deform.cc (Blender 5.2.4,
//! GPL-2.0-or-later). Verified on 5.2.4: names `Group`, `Group.001`, `Group.002` (the last
//! from `''`); `active_index` follows the new group; works in Edit Mode; a camera raises
//! `RuntimeError: Error: VertexGroups.new() is not supported for 'Camera' objects`.

use std::sync::Arc;

use frap_bmesh::BMesh;
use frap_bmesh::shape::deform;
use frap_mesh::Mesh;
use frap_rna::{IdCode, IdRef, MeshId, ObjectId, RnaError, RnaPtr, Seg, Value, rna_func};
use frap_scene::{FuncArgs, Host, Main};
use smol_str::SmolStr;

use super::_common::{arg_str, internal, report_error, root_id};

/// `rna_enum_object_type_items` name of an object type identifier.
pub(crate) fn object_type_name(ty: &str) -> &'static str {
    match ty {
        "MESH" => "Mesh",
        "CURVE" => "Curve",
        "SURFACE" => "Surface",
        "META" => "Metaball",
        "FONT" => "Text",
        "CURVES" => "Hair Curves",
        "POINTCLOUD" => "Point Cloud",
        "VOLUME" => "Volume",
        "GREASEPENCIL" => "Grease Pencil",
        "ARMATURE" => "Armature",
        "LATTICE" => "Lattice",
        "LIGHT" => "Light",
        "LIGHT_PROBE" => "Light Probe",
        "CAMERA" => "Camera",
        "SPEAKER" => "Speaker",
        _ => "Empty",
    }
}

/// The object a vertex-group function is called on.
pub(crate) fn object_of(this: &RnaPtr) -> Result<IdRef<ObjectId>, RnaError> {
    root_id(this)?
        .typed()
        .ok_or_else(|| internal("vertex groups: not an object"))
}

/// The mesh of a mesh object (frap's only type with vertex groups).
pub(crate) fn mesh_of_object(main: &Main, ob: IdRef<ObjectId>) -> Option<IdRef<MeshId>> {
    let d = main.objects.get(ob)?.data?;
    if d.code != IdCode::ME {
        return None;
    }
    let me = d.typed::<MeshId>()?;
    main.meshes.get(me).map(|_| me)
}

/// `check_object_vgroup_support_and_warn`: the mesh, or Blender's report as an error.
pub(crate) fn supported(
    main: &Main,
    ob: IdRef<ObjectId>,
    op: &str,
) -> Result<IdRef<MeshId>, RnaError> {
    mesh_of_object(main, ob).ok_or_else(|| {
        let ty = main.objects.get(ob).map_or("EMPTY", |o| o.object_type());
        report_error(format!(
            "{op} is not supported for '{}' objects",
            object_type_name(ty)
        ))
    })
}

/// The group index a `VertexGroup` pointer names (`None` when the group is gone).
pub(crate) fn group_of(main: &Main, p: &RnaPtr) -> Option<(IdRef<ObjectId>, usize)> {
    let ob = object_of(p).ok()?;
    let me = mesh_of_object(main, ob)?;
    let names = &main.meshes.get(me)?.vertex_group_names;
    let i = match p.path.as_slice() {
        [Seg::Prop("vertex_groups"), Seg::Key(k), ..] => names.iter().position(|n| n == k)?,
        [Seg::Prop("vertex_groups"), Seg::Index(i), ..] => {
            let n = names.len() as i64;
            let i = if *i < 0 { i + n } else { *i };
            usize::try_from(i).ok().filter(|i| *i < names.len())?
        }
        _ => return None,
    };
    Some((ob, i))
}

/// The canonical pointer of group `name` of `ob`.
pub(crate) fn group_ptr(ob: IdRef<ObjectId>, name: &str) -> RnaPtr {
    RnaPtr::id(ob.any(), &frap_spec_rna::OBJECT)
        .child(Seg::Prop("vertex_groups"), &frap_spec_rna::VERTEX_GROUPS)
        .child(Seg::Key(name.into()), &frap_spec_rna::VERTEX_GROUP)
}

/// Whether `me` is in Edit Mode (`BKE_object_is_in_editmode_vgroup`).
pub(crate) fn in_edit_mode(h: &dyn Host, me: IdRef<MeshId>) -> bool {
    h.edit().meshes.contains_key(&me.key())
}

/// Runs `f` on the edit BMesh of `me` (logged as modified, generation bumped, as
/// `OpCtx::bmesh_mut` does).
pub(crate) fn with_bmesh<R>(
    h: &mut dyn Host,
    me: IdRef<MeshId>,
    f: impl FnOnce(&mut BMesh) -> R,
) -> Option<R> {
    {
        let (_, log) = h.main_mut();
        log.modified.insert(me.any());
    }
    let em = h.edit_mut().meshes.get_mut(&me.key())?;
    em.bm_gen += 1;
    em.view = frap_scene::ViewCache::default();
    Some(f(Arc::make_mut(&mut em.bm)))
}

/// `BKE_object_defgroup_add_name`: appends group `name` (made unique) to the mesh of `ob`,
/// makes it active, returns its name.
pub(crate) fn add_group(
    h: &mut dyn Host,
    ob: IdRef<ObjectId>,
    me: IdRef<MeshId>,
    name: &str,
) -> Result<SmolStr, RnaError> {
    let (main, log) = h.main_mut();
    let m = main
        .meshes
        .get_mut(me, log)
        .ok_or_else(|| internal("vertex groups: the mesh was removed"))?;
    let name = deform::unique_group_name(&m.vertex_group_names, None, name);
    m.vertex_group_names.push(name.clone());
    let count = m.vertex_group_names.len() as i32;
    if let Some(o) = main.objects.get_mut(ob, log) {
        o.vertex_groups_active_index = count - 1;
    }
    Ok(name)
}

/// `BKE_object_defgroup_remove`: removes group `g` (its weights, in the edit BMesh in Edit
/// Mode, then its name), shifting the higher groups and the active index
/// (`object_defgroup_remove_common`).
pub(crate) fn remove_group(
    h: &mut dyn Host,
    ob: IdRef<ObjectId>,
    me: IdRef<MeshId>,
    g: usize,
) -> Result<(), RnaError> {
    if in_edit_mode(h, me) {
        with_bmesh(h, me, |bm| deform::remove_group_bmesh(bm, g as u32));
    }
    let edit = in_edit_mode(h, me);
    let (main, log) = h.main_mut();
    let m: &mut Mesh = main
        .meshes
        .get_mut(me, log)
        .ok_or_else(|| internal("vertex groups: the mesh was removed"))?;
    if !edit {
        deform::remove_group_mesh(m, g as u32);
    }
    m.remove_vertex_group_name(g);
    let empty = m.vertex_group_names.is_empty();
    if let Some(o) = main.objects.get_mut(ob, log) {
        // `actdef > def_nr` → `actdef - 1`, then a valid active index while groups remain.
        if o.vertex_groups_active_index + 1 > g as i32 {
            o.vertex_groups_active_index -= 1;
        }
        if !empty && o.vertex_groups_active_index + 1 < 1 {
            o.vertex_groups_active_index = 0;
        }
    }
    Ok(())
}

#[rna_func(srna = "VertexGroups", func = "new")]
fn vertex_groups_new(
    h: &mut dyn Host,
    this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    super::_common::refuse_detached(this, "new")?;
    let ob = object_of(this)?;
    let me = supported(h.main(), ob, "VertexGroups.new()")?;
    let name = arg_str(args, "name").to_owned();
    let name = add_group(h, ob, me, &name)?;
    Ok(Value::Ptr(Box::new(group_ptr(ob, &name))))
}
