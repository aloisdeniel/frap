// SPDX-License-Identifier: GPL-3.0-or-later
//! Renders show modifiers (M4, WP 4.7). The builder extracts each object's `EvalMode::Render`
//! evaluated mesh (`mod.rs`), so the modifier stack reaches the render through `frap-eval` with
//! the render settings: `show_render` instead of `show_viewport`, Subdivision's
//! `render_levels`, Screw's `render_steps` (`frap_modifiers::stack`). This file holds what is
//! specific to evaluated geometry:
//!
//! - **Material slots** ([`slot_materials`]): an evaluated mesh can carry more material slots
//!   than the object's data (a Boolean in TRANSFER mode appends the operands' materials), and
//!   its faces index them. Blender draws with `BKE_object_material_count_eval` slots (the larger
//!   of the object's and the evaluated data's) and resolves each with
//!   `BKE_object_material_get_eval`: an OBJECT-linked slot with a material wins, otherwise the
//!   evaluated data's material of that slot. Face indices past the last slot use the last one
//!   (`mesh.rs`, the draw cache's clamp).
//! - **Mesh sharing**: render meshes are keyed by content (`keys::mesh_key`) and memoised by the
//!   evaluated `Arc` (`Builder::mesh`), so linked duplicates without modifiers (whose evaluated
//!   mesh is the shared original `Arc`) share one `RMesh`, while duplicates whose modifiers make
//!   different geometry get one each (and identical results share again by content).
//!
//! Ported from blender/source/blender/blenkernel/intern/material.cc
//! (`BKE_object_material_count_eval`, `BKE_object_material_get_eval`) and
//! blender/source/blender/draw/engines/workbench/workbench_engine.cc (`mesh_sync`,
//! `get_material`).

use frap_mesh::Mesh;
use frap_rna::{IdRef, MaterialId};
use frap_scene::id::{Object, SlotLink};

/// The material of every slot an object is drawn with: `max(ob.totcol, data_eval.totcol)`
/// slots (`ob.totcol` follows the original data `data`), each the OBJECT-linked material when
/// the slot links to the object and has one, else the evaluated data's material of that slot
/// (`None`: Blender's default surface). `evaluated` is the evaluated mesh (`None`: the data).
pub fn slot_materials(
    ob: &Object,
    data: Option<&Mesh>,
    evaluated: Option<&Mesh>,
) -> Vec<Option<IdRef<MaterialId>>> {
    let totcol = data.map_or(0, |m| m.materials.len());
    let eval_materials: &[Option<IdRef<MaterialId>>] = match evaluated.or(data) {
        Some(m) => &m.materials,
        None => &[],
    };
    let count = totcol.max(eval_materials.len());
    (0..count)
        .map(|i| {
            let linked = (i < totcol)
                .then(|| ob.material_slot_overrides.get(i))
                .flatten()
                .filter(|o| o.link == SlotLink::Object)
                .and_then(|o| o.material);
            linked.or_else(|| eval_materials.get(i).copied().flatten())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use frap_mesh::primitives::cube;
    use frap_mesh::{AttrData, Attribute, Domain};
    use frap_rna::{IdKey, IdRef, MaterialId, MeshId, ObjectId};
    use frap_scene::id::{Material, Object, SlotLink, SlotOverride};
    use frap_scene::{
        Base, BaseFlag, ChangeLog, Context, EditState, EvalError, EvalMode, EvaluatedObject,
        Evaluator, HostRead, Main, Preferences, WindowState,
    };
    use glam::Mat4;

    use super::*;
    use crate::build::build;
    use crate::types::{RCamera, RenderRequest};

    fn mat() -> Option<IdRef<MaterialId>> {
        Some(IdRef::from_key(IdKey::next()))
    }

    fn mesh_with(materials: Vec<Option<IdRef<MaterialId>>>) -> Mesh {
        let mut m = cube(2.0, false, [1.0; 3]);
        m.materials = materials;
        m
    }

    fn linked(ob: &mut Object, i: usize, material: Option<IdRef<MaterialId>>) {
        ob.material_slot_overrides.resize(
            i + 1,
            SlotOverride {
                link: SlotLink::Data,
                material: None,
            },
        );
        ob.material_slot_overrides[i] = SlotOverride {
            link: SlotLink::Object,
            material,
        };
    }

    #[test]
    fn evaluated_slots_extend_the_objects() {
        // A Boolean TRANSFER result: the data's slot, then the operand's material.
        let (a, b, c) = (mat(), mat(), mat());
        let data = mesh_with(vec![a]);
        let ev = mesh_with(vec![a, b]);
        let mut ob = Object::new("Ob", None);
        assert_eq!(slot_materials(&ob, Some(&data), Some(&ev)), vec![a, b]);
        // An OBJECT-linked slot wins inside the object's slots only.
        linked(&mut ob, 0, c);
        assert_eq!(slot_materials(&ob, Some(&data), Some(&ev)), vec![c, b]);
        // Without an evaluated mesh: the data's slots.
        assert_eq!(slot_materials(&ob, Some(&data), None), vec![c]);
    }

    #[test]
    fn empty_object_linked_slot_falls_back_to_the_data() {
        // `if (material != nullptr) return material;` then the data's slot.
        let a = mat();
        let data = mesh_with(vec![a]);
        let mut ob = Object::new("Ob", None);
        linked(&mut ob, 0, None);
        assert_eq!(slot_materials(&ob, Some(&data), Some(&data)), vec![a]);
    }

    #[test]
    fn evaluated_mesh_with_fewer_slots_keeps_the_objects_count() {
        let (a, b) = (mat(), mat());
        let data = mesh_with(vec![a, b]);
        let ev = mesh_with(vec![a]);
        let mut ob = Object::new("Ob", None);
        assert_eq!(slot_materials(&ob, Some(&data), Some(&ev)), vec![a, None]);
        linked(&mut ob, 1, b);
        assert_eq!(slot_materials(&ob, Some(&data), Some(&ev)), vec![a, b]);
        assert!(slot_materials(&ob, None, None).is_empty());
    }

    // -----------------------------------------------------------------------------------------
    // The whole builder with injected evaluated meshes.

    struct Host {
        main: Main,
        win: WindowState,
        edit: EditState,
        prefs: Preferences,
    }

    impl HostRead for Host {
        fn main(&self) -> &Main {
            &self.main
        }
        fn win(&self) -> &WindowState {
            &self.win
        }
        fn edit(&self) -> &EditState {
            &self.edit
        }
        fn ctx(&self) -> Context<'_> {
            Context {
                main: &self.main,
                win: &self.win,
                prefs: &self.prefs,
            }
        }
        fn mesh_view(&self, mesh: IdRef<MeshId>) -> Arc<Mesh> {
            self.main.meshes.get_arc(mesh).expect("mesh exists")
        }
    }

    /// Evaluated meshes by object (the original `Arc` for objects not listed); records the
    /// evaluation mode.
    #[derive(Default)]
    struct Eval {
        meshes: HashMap<IdRef<ObjectId>, Arc<Mesh>>,
        modes: Vec<EvalMode>,
    }

    impl Evaluator for Eval {
        fn object(
            &mut self,
            host: &dyn HostRead,
            ob: IdRef<ObjectId>,
            mode: EvalMode,
        ) -> Result<Arc<EvaluatedObject>, EvalError> {
            self.modes.push(mode);
            let o = host.main().objects.get(ob).unwrap();
            let mesh = self.meshes.get(&ob).cloned().or_else(|| {
                o.data
                    .and_then(|d| d.typed::<MeshId>())
                    .map(|m| host.mesh_view(m))
            });
            Ok(Arc::new(EvaluatedObject {
                matrix_world: Mat4::IDENTITY,
                mesh,
                bounds: None,
                pose: None,
                modifier_status: Vec::new(),
                warnings: Vec::new(),
            }))
        }
        fn depsgraph_update(&mut self, _: &dyn HostRead) {}
    }

    fn host() -> Host {
        let (main, win) = frap_scene::startup::factory();
        Host {
            main,
            win,
            edit: EditState::default(),
            prefs: Preferences::default(),
        }
    }

    fn duplicate(h: &mut Host, name: &str) -> IdRef<ObjectId> {
        let mut log = ChangeLog::default();
        let data = h
            .main
            .objects
            .get(h.main.objects.find("Cube").unwrap())
            .unwrap()
            .data;
        let ob = h.main.objects.insert(name, Object::new("", data), &mut log);
        let coll = h.main.collections.find("Collection").unwrap();
        h.main
            .collections
            .get_mut(coll, &mut log)
            .unwrap()
            .objects
            .push(ob);
        let sc = h.win.scene;
        h.main.scenes.get_mut(sc, &mut log).unwrap().view_layers[0]
            .bases
            .push(Base {
                object: ob,
                flag: BaseFlag::empty(),
            });
        ob
    }

    fn request() -> RenderRequest {
        RenderRequest {
            resolution: Some([8, 8]),
            camera: Some(RCamera::default()),
            ..RenderRequest::default()
        }
    }

    #[test]
    fn renders_evaluate_in_render_mode_and_share_meshes_by_content() {
        let mut h = host();
        duplicate(&mut h, "Twin");
        let sub_a = duplicate(&mut h, "SubA");
        let sub_b = duplicate(&mut h, "SubB");
        let other = duplicate(&mut h, "Other");
        let data = h.main.meshes.find("Cube").unwrap();
        let mats = h.main.meshes.get(data).unwrap().materials.clone();
        // Two objects whose stacks give the same (smaller) cube, one a different size.
        let small = Arc::new(mesh_with(mats.clone()));
        let mut big = cube(4.0, false, [1.0; 3]);
        big.materials = mats;
        let mut eval = Eval::default();
        eval.meshes.insert(sub_a, small.clone());
        eval.meshes.insert(sub_b, Arc::new((*small).clone()));
        eval.meshes.insert(other, Arc::new(big));
        let rs = build(&h, &mut eval, &request()).unwrap();
        assert!(eval.modes.iter().all(|m| *m == EvalMode::Render));
        let key = |name: &str| {
            rs.objects
                .iter()
                .find(|o| o.name == name)
                .unwrap()
                .mesh
                .unwrap()
        };
        // Linked duplicates without modifiers share the data's mesh; equal evaluated
        // geometry shares by content; different geometry does not.
        assert_eq!(key("Cube"), key("Twin"));
        assert_eq!(key("SubA"), key("SubB"));
        assert_ne!(key("Cube"), key("SubA"));
        assert_ne!(key("SubA"), key("Other"));
        assert_ne!(key("Cube"), key("Other"));
        assert_eq!(rs.meshes.len(), 3);
    }

    #[test]
    fn materials_added_by_modifiers_are_drawn() {
        let mut h = host();
        let mut log = ChangeLog::default();
        let cube_ob = h.main.objects.find("Cube").unwrap();
        let data = h.main.meshes.find("Cube").unwrap();
        let red = h.main.materials.insert("Red", Material::new(""), &mut log);
        let orig = h.main.meshes.get(data).unwrap().materials.clone();
        assert_eq!(orig.len(), 1);
        // The evaluated mesh appends 'Red' and puts faces 3.. in slot 1.
        let mut ev = mesh_with(vec![orig[0], Some(red)]);
        ev.attributes_mut().insert(Attribute {
            name: crate::build::mesh::MATERIAL_INDEX.into(),
            domain: Domain::Face,
            data: AttrData::Int(Arc::new(vec![0, 0, 0, 1, 1, 1])),
        });
        let mut eval = Eval::default();
        eval.meshes.insert(cube_ob, Arc::new(ev));
        let rs = build(&h, &mut eval, &request()).unwrap();
        let ob = rs.objects.iter().find(|o| o.name == "Cube").unwrap();
        assert_eq!(ob.materials.len(), 2);
        assert!(ob.materials.iter().all(Option::is_some));
        assert_ne!(ob.materials[0], ob.materials[1]);
        let rm = &rs.meshes[&ob.mesh.unwrap()];
        let slots: Vec<u32> = rm.submeshes.iter().map(|s| s.0).collect();
        assert_eq!(slots, vec![0, 1]);
    }
}
