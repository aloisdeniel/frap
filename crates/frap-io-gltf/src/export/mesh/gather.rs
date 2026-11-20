// SPDX-License-Identifier: GPL-3.0-or-later
//! Mesh objects → glTF meshes: runs the primitive driver once per mesh datablock, writes each
//! primitive's accessors, and builds one glTF mesh per (mesh, material slots) pair.
//!
//! Ported from blender/scripts/addons_core/io_scene_gltf2/blender/exp/nodes.py
//! (`__gather_mesh`: `materials = tuple(ms.material for ms in material_slots)`), obj_data.py
//! (`gather_mesh`, cache key `(id(mesh), modifiers, materials)`; name = mesh name; a mesh
//! without primitives is omitted with a warning), primitives.py (`gather_primitives`,
//! `__gather_cache_primitives` keyed by the mesh only, so objects that share a mesh but not
//! its materials share the accessors; `__gather_indices`), primitive_attributes.py
//! (POSITION is the only attribute with min/max; normalized integer attributes) and
//! material/materials.py (`get_material_from_idx`: an index past the last slot uses the last
//! slot; no material for `PLACEHOLDER`/`NONE`).
//!
//! `export_apply=True` (M4, WP 4.7; reassigned): an object with modifiers exports its evaluated
//! mesh as a glTF mesh of its own, with the evaluated mesh's materials; objects without
//! modifiers keep sharing their data's mesh (rules and provenance in `export/apply.rs`).

use std::collections::HashMap;

use frap_gltf::json::{Index, Mesh as GMesh, Mode, Primitive};
use frap_gltf::{AccessorData, AccessorOpts};
use frap_mesh::Mesh;
use frap_rna::{IdRef, MaterialId, MeshId, ObjectId, SceneId};
use frap_scene::id::{Object, SlotLink};
use indexmap::IndexMap;

use crate::export::gather::Gather;
use crate::export::mesh::ext::{AttrColumn, AttrValues, MeshExportCx, PrimitiveDraft};
use crate::export::mesh::primitives::{self, NO_MATERIAL_SLOT};
use crate::export::{ExportError, ExportMaterials, apply, materials};

/// A primitive whose accessors are in the buffer (reused by every glTF mesh of its mesh).
#[derive(Clone, Debug)]
pub struct WrittenPrimitive {
    /// Attribute accessors in write order.
    pub attributes: IndexMap<String, Index>,
    /// Index accessor.
    pub indices: Index,
    /// glTF mode.
    pub mode: u32,
    /// Material slot index (`NO_MATERIAL_SLOT` with `export_materials='NONE'`).
    pub material_slot: usize,
    /// Morph target accessors.
    pub targets: Vec<IndexMap<String, Index>>,
}

/// The object's material per slot (`ob.material_slots[i].material`): the object's own material
/// for `link == 'OBJECT'` slots, the mesh's otherwise.
pub fn slot_materials(ob: &Object, mesh: &Mesh) -> Vec<Option<IdRef<MaterialId>>> {
    (0..mesh.materials.len())
        .map(|i| match ob.material_slot_overrides.get(i) {
            Some(o) if o.link == SlotLink::Object => o.material,
            _ => mesh.materials[i],
        })
        .collect()
}

/// `get_material_from_idx`.
pub fn material_for_slot(
    settings_materials: ExportMaterials,
    slot: usize,
    materials: &[Option<IdRef<MaterialId>>],
) -> Option<IdRef<MaterialId>> {
    if !matches!(
        settings_materials,
        ExportMaterials::Export | ExportMaterials::Viewport
    ) || slot == NO_MATERIAL_SLOT
        || materials.is_empty()
    {
        return None;
    }
    let i = slot.min(materials.len() - 1);
    materials[i]
}

/// Accessor data of one attribute column.
pub fn column_data(c: &AttrColumn) -> AccessorData {
    let d = match &c.values {
        AttrValues::F32x2(v) => AccessorData::from_elements(v),
        AttrValues::F32x3(v) => AccessorData::from_elements(v),
        AttrValues::F32x4(v) => AccessorData::from_elements(v),
        AttrValues::U8x4(v) => AccessorData::from_elements(v),
        AttrValues::U16x4(v) => AccessorData::from_elements(v),
    };
    AccessorData {
        normalized: c.normalized && d.component_type().can_normalize(),
        ..d
    }
}

/// An applied glTF mesh's identity: the scene, the object and its material per slot.
type AppliedKey = (
    IdRef<SceneId>,
    IdRef<ObjectId>,
    Vec<Option<IdRef<MaterialId>>>,
);

/// glTF meshes of `export_apply` objects, per (scene, object, materials): an object with
/// modifiers never shares its mesh with another object (`apply.rs`), nor with its own node of
/// another scene (each scene's depsgraph gives a temporary mesh of its own; verified on 5.2.4:
/// a Cube with a Subdivision linked into three scenes exports three glTF meshes).
#[derive(Default)]
struct AppliedMeshes(HashMap<AppliedKey, Option<Index>>);

/// glTF meshes of objects with an ARMATURE modifier under `export_skins` (WP 6.8), per
/// (object, materials): the add-on's cache key holds the object's modifiers, so such an object
/// never shares its mesh, and a skinned mesh is in world space.
#[derive(Default)]
struct SkinnedMeshes(HashMap<SkinnedKey, Option<Index>>);

/// A skinned glTF mesh's identity: the object and its material per slot.
type SkinnedKey = (IdRef<ObjectId>, Vec<Option<IdRef<MaterialId>>>);

/// The glTF mesh of a MESH object (`None` for other types and meshes without primitives).
///
/// With `export_apply=True` an object with modifiers exports its evaluated mesh as its own glTF
/// mesh ([`apply::evaluated`]); every other object exports its data, shared by (mesh,
/// materials).
pub fn gather_mesh(g: &mut Gather, ob: &Object) -> Result<Option<Index>, ExportError> {
    gather_mesh_with(g, ob, None)
}

/// [`gather_mesh`] for an object whose mesh is its own (`per_object`: the object and, when its
/// mesh is skinned, the skin; WP 6.8, `export/skins.rs`).
pub fn gather_mesh_with(
    g: &mut Gather,
    ob: &Object,
    per_object: Option<(IdRef<ObjectId>, Option<crate::export::skins::MeshSkin>)>,
) -> Result<Option<Index>, ExportError> {
    // `--stmt-timeout` cancellation point, once per exported object (§12).
    frap_math::budget::check_now();
    if ob.object_type() != "MESH" {
        return Ok(None);
    }
    let Some(me_ref) = ob.data.and_then(|d| d.typed::<MeshId>()) else {
        return Ok(None);
    };
    let main = g.main;
    let Some(mesh) = main.meshes.get(me_ref) else {
        return Ok(None);
    };
    if g.settings.evaluated_meshes.is_some()
        && apply::has_modifiers(ob)
        && let Some(ob_ref) = main.objects.find_in(&ob.header.name, ob.header.library)
    {
        // Not evaluated (outside the viewport depsgraph, or its evaluation failed): `to_mesh()`
        // is a copy of the data, still a glTF mesh of the object's own.
        let evaluated = apply::evaluated(g, ob_ref);
        let data: &Mesh = evaluated.as_deref().unwrap_or(mesh);
        let mats = apply::evaluated_materials(ob, mesh, data);
        let key = (g.scene, ob_ref, mats);
        if let Some(i) = g.state::<AppliedMeshes>().0.get(&key) {
            return Ok(*i);
        }
        let skin = per_object.as_ref().and_then(|(_, s)| s.as_ref());
        let (index, _) = build_mesh(g, data, mesh, Some(ob_ref), &key.2, None, skin)?;
        g.state::<AppliedMeshes>().0.insert(key, index);
        return Ok(index);
    }
    let mats = slot_materials(ob, mesh);
    if let Some((ob_ref, skin)) = per_object {
        let key = (ob_ref, mats);
        if let Some(i) = g.state::<SkinnedMeshes>().0.get(&key) {
            return Ok(*i);
        }
        let (index, _) = build_mesh(g, mesh, mesh, Some(ob_ref), &key.1, None, skin.as_ref())?;
        g.state::<SkinnedMeshes>().0.insert(key, index);
        return Ok(index);
    }
    let key = (me_ref, mats);
    if let Some(i) = g.meshes.get(&key) {
        return Ok(*i);
    }
    let cached = g.primitives.get(&me_ref).cloned();
    let (index, written) = build_mesh(g, mesh, mesh, None, &key.1, cached, None)?;
    if let Some(w) = written {
        g.primitives.insert(me_ref, w);
    }
    g.meshes.insert(key, index);
    Ok(index)
}

/// What [`build_mesh`] returns: the glTF mesh (`None` without primitives) and, when the
/// primitives were extracted by this call, what was written (for the per-mesh cache).
type Built = (Option<Index>, Option<Vec<WrittenPrimitive>>);

/// Builds one glTF mesh from `data` (the original mesh, or an object's evaluated mesh) with the
/// given material per slot, named after (and with the `extras` of) the original data `orig`.
/// `cached`: the primitives already written for `data` (their accessors are reused).
fn build_mesh(
    g: &mut Gather,
    data: &Mesh,
    orig: &Mesh,
    object: Option<IdRef<ObjectId>>,
    mats: &[Option<IdRef<MaterialId>>],
    cached: Option<Vec<WrittenPrimitive>>,
    skin: Option<&crate::export::skins::MeshSkin>,
) -> Result<Built, ExportError> {
    let (main, settings) = (g.main, g.settings);
    // Primitives are extracted and written once per mesh (the add-on's
    // `__gather_cache_primitives`); each is followed by its material, in traversal order.
    enum Source {
        Fresh(std::vec::IntoIter<PrimitiveDraft>),
        Cached(std::vec::IntoIter<WrittenPrimitive>),
    }
    let mut source = match cached {
        Some(w) => Source::Cached(w.into_iter()),
        None => {
            let cx = MeshExportCx::new(main, data, object, settings).with_skin(skin);
            Source::Fresh(primitives::extract(&cx)?.into_iter())
        }
    };
    let fresh = matches!(source, Source::Fresh(_));
    let mut out: Vec<Primitive> = Vec::new();
    let mut written_all: Vec<WrittenPrimitive> = Vec::new();
    loop {
        let written = match &mut source {
            Source::Fresh(drafts) => {
                let Some(mut d) = drafts.next() else { break };
                let cx = MeshExportCx::new(main, data, object, settings).with_skin(skin);
                primitives::post_process(&cx, &mut d, &mut g.buf)?;
                let w = write_accessors(g, &d)?;
                written_all.push(w.clone());
                w
            }
            Source::Cached(w) => match w.next() {
                Some(w) => w,
                None => break,
            },
        };
        let mat = material_for_slot(settings.export_materials, written.material_slot, mats);
        let material = match mat {
            Some(m) => materials::gather(g, m, data)?,
            None => None,
        };
        out.push(Primitive {
            attributes: written.attributes,
            indices: Some(written.indices),
            material,
            mode: (written.mode != 4)
                .then(|| Mode::try_from(written.mode).ok())
                .flatten(),
            targets: written.targets,
            ..Primitive::default()
        });
    }
    let index = if out.is_empty() {
        g.warn(format!(
            "Mesh '{}' has no primitives and will be omitted.",
            orig.header.name
        ));
        None
    } else {
        let extras = crate::export::extras::mesh(g, orig);
        g.doc.meshes.push(GMesh {
            name: Some(orig.header.name.to_string()),
            primitives: out,
            extras,
            ..GMesh::default()
        });
        Some((g.doc.meshes.len() - 1) as Index)
    };
    Ok((index, fresh.then_some(written_all)))
}

/// Writes one primitive's accessors in the add-on's order: POSITION (with min/max), NORMAL,
/// TEXCOORD_n, every further attribute, then the indices (shared when identical), then the
/// morph targets.
fn write_accessors(g: &mut Gather, d: &PrimitiveDraft) -> Result<WrittenPrimitive, ExportError> {
    let n = d.positions.len();
    let check = |name: &str, len: usize| -> Result<(), ExportError> {
        if len == n {
            Ok(())
        } else {
            Err(ExportError::Invalid(format!(
                "attribute {name} has {len} values for {n} vertices"
            )))
        }
    };
    let mut attributes = IndexMap::new();
    let pos = g.buf.push_accessor(
        &AccessorData::from_elements(&d.positions),
        &AccessorOpts::attribute(true),
    );
    attributes.insert("POSITION".to_owned(), pos);
    if !d.normals.is_empty() {
        check("NORMAL", d.normals.len())?;
        let i = g.buf.push_accessor(
            &AccessorData::from_elements(&d.normals),
            &AccessorOpts::attribute(false),
        );
        attributes.insert("NORMAL".to_owned(), i);
    }
    for (k, tc) in d.texcoords.iter().enumerate() {
        check("TEXCOORD", tc.len())?;
        let i = g.buf.push_accessor(
            &AccessorData::from_elements(tc),
            &AccessorOpts::attribute(false),
        );
        attributes.insert(format!("TEXCOORD_{k}"), i);
    }
    for (name, col) in &d.attributes {
        let data = column_data(col);
        check(name, data.count())?;
        let i = g.buf.push_accessor(&data, &AccessorOpts::attribute(false));
        attributes.insert(name.to_string(), i);
    }
    let indices = match g.index_accessors.get(&d.indices) {
        Some(i) => *i,
        None => {
            if let Some(bad) = d.indices.iter().find(|i| **i as usize >= n) {
                return Err(ExportError::Invalid(format!(
                    "index {bad} out of range ({n} vertices)"
                )));
            }
            let i = g.buf.push_indices(&d.indices);
            g.index_accessors.insert(d.indices.clone(), i);
            i
        }
    };
    let mut targets = Vec::with_capacity(d.targets.len());
    for t in &d.targets {
        let mut m = IndexMap::new();
        for (name, col) in t {
            let data = column_data(col);
            check(name, data.count())?;
            // Morph targets may be sparse (WP 6.8, `post/morph_targets.rs`).
            let i = crate::export::mesh::post::morph_targets::push_target_accessor(
                &mut g.buf, g.settings, name, &data,
            )?;
            m.insert(name.to_string(), i);
        }
        targets.push(m);
    }
    Ok(WrittenPrimitive {
        attributes,
        indices,
        mode: d.mode,
        material_slot: d.material_slot,
        targets,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn material_index_past_last_slot_uses_last() {
        let a = Some(IdRef::<MaterialId>::from_key(frap_rna::IdKey::next()));
        let b = Some(IdRef::<MaterialId>::from_key(frap_rna::IdKey::next()));
        let mats = [a, b];
        assert_eq!(material_for_slot(ExportMaterials::Export, 0, &mats), a);
        assert_eq!(material_for_slot(ExportMaterials::Export, 7, &mats), b);
        assert_eq!(material_for_slot(ExportMaterials::Viewport, 1, &mats), b);
        assert_eq!(
            material_for_slot(ExportMaterials::Placeholder, 1, &mats),
            None
        );
        assert_eq!(material_for_slot(ExportMaterials::None, 1, &mats), None);
        assert_eq!(material_for_slot(ExportMaterials::Export, 0, &[]), None);
        assert_eq!(
            material_for_slot(ExportMaterials::Export, NO_MATERIAL_SLOT, &mats),
            None
        );
    }

    #[test]
    fn ushort_colours_are_normalized() {
        let c = AttrColumn {
            values: AttrValues::U16x4(vec![[65535, 0, 32768, 65535]]),
            normalized: true,
        };
        let d = column_data(&c);
        assert!(d.normalized);
        assert_eq!(
            d.component_type(),
            frap_gltf::json::ComponentType::UnsignedShort
        );
        assert_eq!(d.type_, frap_gltf::json::AccessorType::Vec4);
        // FLOAT cannot be normalized.
        let f = column_data(&AttrColumn {
            values: AttrValues::F32x4(vec![[1.0; 4]]),
            normalized: true,
        });
        assert!(!f.normalized);
    }

    /// `export_apply` sharing rules (WP 4.7), on a scene built in Rust: the factory 'Cube' and
    /// linked duplicates of its mesh, with the evaluated meshes injected as the operator would
    /// pass them. Expected structure read from Blender 5.2.4 on the same scene
    /// (`tests/golden/cases/modifiers/integration/export_apply_sharing.py`).
    mod apply_sharing {
        use std::collections::HashMap;
        use std::sync::Arc;

        use frap_gltf::Document;
        use frap_mesh::Mesh;
        use frap_mesh::primitives::cube;
        use frap_rna::{IdRef, MaterialId, ObjectId};
        use frap_scene::generated::modifier_settings::ModifierType;
        use frap_scene::id::{
            Material, Modifier, ModifierSettings, Object, SlotLink, SlotOverride,
        };
        use frap_scene::{Base, BaseFlag, ChangeLog, Context, Main, Preferences, WindowState};

        use crate::export::{ExportSettings, export};

        struct S {
            main: Main,
            win: WindowState,
            prefs: Preferences,
            log: ChangeLog,
        }

        impl S {
            fn factory() -> S {
                let (main, win) = frap_scene::startup::factory();
                S {
                    main,
                    win,
                    prefs: Preferences::default(),
                    log: ChangeLog::default(),
                }
            }
            fn ob(&self, name: &str) -> IdRef<ObjectId> {
                self.main.objects.find(name).unwrap()
            }
            fn duplicate(&mut self, name: &str) -> IdRef<ObjectId> {
                let data = self.main.objects.get(self.ob("Cube")).unwrap().data;
                let ob = self
                    .main
                    .objects
                    .insert(name, Object::new("", data), &mut self.log);
                let coll = self.main.collections.find("Collection").unwrap();
                let c = self.main.collections.get_mut(coll, &mut self.log).unwrap();
                c.objects.push(ob);
                let sc = self.win.scene;
                let scene = self.main.scenes.get_mut(sc, &mut self.log).unwrap();
                scene.view_layers[0].bases.push(Base {
                    object: ob,
                    flag: BaseFlag::empty(),
                });
                ob
            }
            fn modifier(&mut self, ob: IdRef<ObjectId>, ty: ModifierType, show_viewport: bool) {
                let o = self.main.objects.get_mut(ob, &mut self.log).unwrap();
                o.modifiers.push(Modifier {
                    name: "M".into(),
                    show_viewport,
                    show_render: true,
                    show_in_editmode: false,
                    show_on_cage: false,
                    show_expanded: true,
                    is_active: true,
                    use_pin_to_last: false,
                    persistent_uid: 1,
                    settings: ModifierSettings::new(ty),
                });
            }
            fn link_object_material(&mut self, ob: IdRef<ObjectId>, m: IdRef<MaterialId>) {
                let o = self.main.objects.get_mut(ob, &mut self.log).unwrap();
                o.material_slot_overrides = vec![SlotOverride {
                    link: SlotLink::Object,
                    material: Some(m),
                }];
            }
            fn export(&self, evaluated: Option<HashMap<IdRef<ObjectId>, Arc<Mesh>>>) -> Document {
                // The operator's map is per (scene, object); these tests export one scene.
                let evaluated = evaluated.map(|m| {
                    m.into_iter()
                        .map(|(o, me)| ((self.win.scene, o), me))
                        .collect::<crate::export::settings::EvaluatedMeshes>()
                });
                let ctx = Context {
                    main: &self.main,
                    win: &self.win,
                    prefs: &self.prefs,
                };
                let settings = ExportSettings {
                    generator: "frap test".to_owned(),
                    evaluated_meshes: evaluated.map(Arc::new),
                    ..ExportSettings::default()
                };
                export(&self.main, &ctx, &settings).unwrap().document
            }
        }

        fn mesh_of(d: &Document, node: &str) -> Option<usize> {
            let n = d.nodes.iter().find(|n| n.name.as_deref() == Some(node));
            n.unwrap().mesh.map(|m| m as usize)
        }

        fn material_name(d: &Document, mesh: usize) -> String {
            let m = d.meshes[mesh].primitives[0].material.unwrap() as usize;
            d.materials[m].name.clone().unwrap()
        }

        /// The scene of the golden: 'Cube' (no modifiers), 'Cube.001' (Subdivision),
        /// 'Cube.002' (a Bevel off in the viewport, an OBJECT-linked 'Blue'), 'F' (no
        /// modifiers, an OBJECT-linked 'Blue').
        fn scene() -> S {
            let mut s = S::factory();
            let blue = s
                .main
                .materials
                .insert("Blue", Material::new(""), &mut s.log);
            let a = s.duplicate("Cube.001");
            let b = s.duplicate("Cube.002");
            let f = s.duplicate("F");
            s.modifier(a, ModifierType::Subsurf, true);
            s.modifier(b, ModifierType::Bevel, false);
            s.link_object_material(b, blue);
            s.link_object_material(f, blue);
            s
        }

        fn evaluated(s: &S) -> HashMap<IdRef<ObjectId>, Arc<Mesh>> {
            let orig = s.main.meshes.find("Cube").unwrap();
            let orig = s.main.meshes.get_arc(orig).unwrap();
            let mut sub = cube(1.0, true, [1.0; 3]);
            sub.materials = orig.materials.clone();
            // Objects without modifiers are in the map too (the operator evaluates every MESH
            // object): an empty mesh shows if one were used.
            [
                ("Cube", Arc::new(Mesh::new(""))),
                ("Cube.001", Arc::new(sub)),
                ("Cube.002", Arc::new((*orig).clone())),
                ("F", Arc::new(Mesh::new(""))),
            ]
            .into_iter()
            .map(|(n, m)| (s.ob(n), m))
            .collect()
        }

        #[test]
        fn objects_with_modifiers_get_their_own_mesh() {
            let s = scene();
            let d = s.export(Some(evaluated(&s)));
            let [cube, a, b, f] =
                ["Cube", "Cube.001", "Cube.002", "F"].map(|n| mesh_of(&d, n).unwrap());
            assert_eq!(d.meshes.len(), 4);
            // Every glTF mesh is named after the data.
            assert!(d.meshes.iter().all(|m| m.name.as_deref() == Some("Cube")));
            // No modifiers: the data, shared accessors, the object's own materials.
            let attr = |m: usize, k: &str| d.meshes[m].primitives[0].attributes[k];
            let indices = |m: usize| d.meshes[m].primitives[0].indices.unwrap();
            assert_ne!(cube, f);
            assert_eq!(attr(cube, "POSITION"), attr(f, "POSITION"));
            assert_eq!(material_name(&d, cube), "Material");
            assert_eq!(material_name(&d, f), "Blue");
            // Subdivision: the evaluated mesh (24 corner-split vertices of the stand-in cube,
            // at half size).
            let pos = attr(a, "POSITION") as usize;
            assert_eq!(d.accessors[pos].max.as_ref().unwrap()[0], 0.5);
            // A disabled modifier still makes a mesh of its own: fresh vertex accessors,
            // the index accessor shared by content, and the evaluated mesh's material (the
            // OBJECT-linked slot is not applied).
            assert_ne!(attr(b, "POSITION"), attr(cube, "POSITION"));
            assert_ne!(attr(b, "NORMAL"), attr(cube, "NORMAL"));
            assert_eq!(indices(b), indices(cube));
            assert_eq!(material_name(&d, b), "Material");
            let mut all = vec![cube, a, b, f];
            all.dedup();
            assert_eq!(all.len(), 4);
        }

        #[test]
        fn without_export_apply_linked_duplicates_share() {
            let s = scene();
            let d = s.export(None);
            let [cube, a, b, f] =
                ["Cube", "Cube.001", "Cube.002", "F"].map(|n| mesh_of(&d, n).unwrap());
            // (mesh, materials): 'Cube' and 'Cube.001' share one glTF mesh; 'Cube.002' and
            // 'F' (both OBJECT-linked 'Blue') share another, with the same accessors.
            assert_eq!(d.meshes.len(), 2);
            assert_eq!(cube, a);
            assert_eq!(b, f);
            assert_eq!(material_name(&d, b), "Blue");
            assert_eq!(
                d.meshes[cube].primitives[0].attributes,
                d.meshes[f].primitives[0].attributes
            );
        }

        #[test]
        fn empty_evaluated_mesh_is_omitted_with_a_warning() {
            let mut s = S::factory();
            let ob = s.ob("Cube");
            s.modifier(ob, ModifierType::Bevel, true);
            let ctx = Context {
                main: &s.main,
                win: &s.win,
                prefs: &s.prefs,
            };
            let settings = ExportSettings {
                generator: "frap test".to_owned(),
                evaluated_meshes: Some(Arc::new(HashMap::from([(
                    (s.win.scene, ob),
                    Arc::new(Mesh::new("")),
                )]))),
                ..ExportSettings::default()
            };
            let out = export(&s.main, &ctx, &settings).unwrap();
            assert!(out.document.meshes.is_empty());
            assert_eq!(mesh_of(&out.document, "Cube"), None);
            assert!(
                out.reports
                    .iter()
                    .any(|r| r.message == "Mesh 'Cube' has no primitives and will be omitted.")
            );
        }

        #[test]
        fn object_without_evaluation_exports_its_data_as_its_own_mesh() {
            // The operator leaves out objects outside the viewport depsgraph (and objects whose
            // evaluation failed): `to_mesh()` is their data, but an object with modifiers still
            // gets a glTF mesh of its own, with fresh vertex accessors and the data's materials
            // (verified on 5.2.4: export_apply_hidden.py).
            let s = scene();
            let d = s.export(Some(HashMap::new()));
            let [cube, a, b, f] =
                ["Cube", "Cube.001", "Cube.002", "F"].map(|n| mesh_of(&d, n).unwrap());
            assert_eq!(d.meshes.len(), 4);
            let attr = |m: usize, k: &str| d.meshes[m].primitives[0].attributes[k];
            let indices = |m: usize| d.meshes[m].primitives[0].indices.unwrap();
            assert_eq!(attr(cube, "POSITION"), attr(f, "POSITION"));
            for m in [a, b] {
                assert_ne!(attr(m, "POSITION"), attr(cube, "POSITION"));
                assert_eq!(indices(m), indices(cube));
                // The data (the factory cube, 1.0), not a subdivided mesh.
                let pos = attr(m, "POSITION") as usize;
                assert_eq!(d.accessors[pos].max.as_ref().unwrap()[0], 1.0);
                assert_eq!(d.accessors[pos].count, 24);
                assert_eq!(material_name(&d, m), "Material");
            }
            assert_ne!(attr(a, "POSITION"), attr(b, "POSITION"));
        }

        /// Per node with a mesh: its glTF mesh index and, per primitive, the accessor index of
        /// every attribute, the index accessor and the material.
        type Sharing = Vec<(
            String,
            usize,
            Vec<(Vec<(String, u32)>, Option<u32>, Option<u32>)>,
        )>;

        fn sharing(d: &Document) -> Sharing {
            let mut out: Sharing = d
                .nodes
                .iter()
                .filter_map(|n| Some((n.name.clone()?, n.mesh? as usize)))
                .map(|(name, m)| {
                    let prims = d.meshes[m]
                        .primitives
                        .iter()
                        .map(|p| {
                            let attrs = p.attributes.iter().map(|(k, v)| (k.clone(), *v)).collect();
                            (attrs, p.indices, p.material)
                        })
                        .collect();
                    (name, m, prims)
                })
                .collect();
            out.sort_by(|a, b| a.0.cmp(&b.0));
            out
        }

        #[test]
        fn accessor_sharing_matches_blender_for_unevaluated_objects() {
            // The scene of `export_apply_hidden.py` at its first export: 'Cube' (Subdivision,
            // OBJECT-linked 'Blue', hidden: not evaluated) and its linked duplicate 'Cube.001'
            // (no modifiers). The accessor indices (vertex accessors per object, the index
            // accessor shared by content) are compared with Blender's file, which the glb
            // golden comparator does not do (it inlines accessor data).
            let mut s = S::factory();
            let blue = s
                .main
                .materials
                .insert("Blue", Material::new(""), &mut s.log);
            s.duplicate("Cube.001");
            let cube = s.ob("Cube");
            s.modifier(cube, ModifierType::Subsurf, true);
            s.link_object_material(cube, blue);
            let d = s.export(Some(HashMap::new()));
            let path = concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/golden/fixtures/modifiers/integration/",
                "export_apply_hidden.out_object.glb"
            );
            let blender = frap_gltf::read(&std::fs::read(path).unwrap())
                .unwrap()
                .document;
            assert_eq!(sharing(&d), sharing(&blender));
            assert_eq!(d.accessors.len(), blender.accessors.len());
            for (a, b) in d.accessors.iter().zip(&blender.accessors) {
                assert_eq!(a.count, b.count);
            }
        }

        fn set_object_hidden(s: &mut S, name: &str) {
            let ob = s.ob(name);
            s.main
                .objects
                .get_mut(ob, &mut s.log)
                .unwrap()
                .hide_viewport = true;
        }

        /// The MESH objects of `viewport_evaluated_objects`, sorted by name.
        fn in_depsgraph(s: &S) -> Vec<String> {
            let ctx = Context {
                main: &s.main,
                win: &s.win,
                prefs: &s.prefs,
            };
            let set = crate::export::apply::viewport_evaluated_objects(&s.main, &ctx);
            let mut names: Vec<String> = set
                .iter()
                .map(|o| s.main.objects.get(*o).unwrap())
                .filter(|o| o.object_type() == "MESH")
                .map(|o| o.header.name.to_string())
                .collect();
            names.sort();
            names
        }

        #[test]
        fn viewport_depsgraph_skips_objects_disabled_in_viewports() {
            let mut s = scene();
            assert_eq!(in_depsgraph(&s), ["Cube", "Cube.001", "Cube.002", "F"]);
            // `Object.hide_viewport`.
            set_object_hidden(&mut s, "Cube.001");
            assert_eq!(in_depsgraph(&s), ["Cube", "Cube.002", "F"]);
            // `LayerCollection.hide_viewport` and `hide_set` still evaluate.
            let sc = s.win.scene;
            let scene = s.main.scenes.get_mut(sc, &mut s.log).unwrap();
            scene.view_layers[0].layer_collection.children[0].hide_viewport = true;
            for b in &mut scene.view_layers[0].bases {
                b.flag.insert(BaseFlag::HIDDEN);
            }
            assert_eq!(in_depsgraph(&s), ["Cube", "Cube.002", "F"]);
            // `Collection.hide_viewport`, also of a parent collection.
            let coll = s.main.collections.find("Collection").unwrap();
            s.main
                .collections
                .get_mut(coll, &mut s.log)
                .unwrap()
                .hide_viewport = true;
            assert!(in_depsgraph(&s).is_empty());
            // Linked from a visible collection too (the scene's master collection): evaluated.
            let cube = s.ob("Cube");
            let scene = s.main.scenes.get_mut(sc, &mut s.log).unwrap();
            scene.collection.objects.push(cube);
            assert_eq!(in_depsgraph(&s), ["Cube"]);
            // An excluded layer collection.
            let mut s = scene_with_hidden_operand(true);
            let sc = s.win.scene;
            let scene = s.main.scenes.get_mut(sc, &mut s.log).unwrap();
            scene.view_layers[0].layer_collection.children[0].exclude = true;
            assert!(in_depsgraph(&s).is_empty());
        }

        /// The factory scene plus a hidden 'Op' (Subdivision), the operand of a Boolean of
        /// 'Cube' enabled in the viewport or not.
        fn scene_with_hidden_operand(enabled: bool) -> S {
            let mut s = S::factory();
            let op = s.duplicate("Op");
            s.modifier(op, ModifierType::Subsurf, true);
            set_object_hidden(&mut s, "Op");
            let cube = s.ob("Cube");
            s.modifier(cube, ModifierType::Boolean, enabled);
            let o = s.main.objects.get_mut(cube, &mut s.log).unwrap();
            if let ModifierSettings::Boolean(b) = &mut o.modifiers[0].settings {
                b.object = Some(op.any());
            }
            s
        }

        #[test]
        fn viewport_depsgraph_evaluates_hidden_geometry_dependencies() {
            // A hidden Boolean operand is evaluated (its geometry is read), unless the Boolean
            // is disabled in the viewport or its owner is not evaluated either.
            assert_eq!(
                in_depsgraph(&scene_with_hidden_operand(true)),
                ["Cube", "Op"]
            );
            assert_eq!(in_depsgraph(&scene_with_hidden_operand(false)), ["Cube"]);
            let mut s = scene_with_hidden_operand(true);
            set_object_hidden(&mut s, "Cube");
            assert!(in_depsgraph(&s).is_empty());
            // A collection operand: every object of the collection.
            let mut s = scene_with_hidden_operand(true);
            let coll = s.main.collections.find("Collection").unwrap();
            let cube = s.ob("Cube");
            let o = s.main.objects.get_mut(cube, &mut s.log).unwrap();
            if let ModifierSettings::Boolean(b) = &mut o.modifiers[0].settings {
                b.operand_type = "COLLECTION".into();
                b.collection = Some(coll.any());
            }
            assert_eq!(in_depsgraph(&s), ["Cube", "Op"]);
        }
    }
}
