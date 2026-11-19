// SPDX-License-Identifier: GPL-3.0-or-later
//! Evaluated scene → `RenderScene` (§19.1): corner-split meshes, `LoweredMaterial`s, images,
//! lights, camera (`BKE_camera_params` port), world, settings. WP 1.15.
//!
//! [`build`] only reads: it takes the state as `&dyn HostRead` (the render operator passes the
//! snapshot of `Host::snapshot_at`, a COW clone with animation applied) and evaluates objects
//! through the snapshot's own `Evaluator` in `EvalMode::Render`. Nothing it does can reach the
//! session's state.
//!
//! Determinism: objects and lights are sorted by name (then library path); meshes, materials and
//! images are inserted in that order; keys are content hashes (meshes) or `(IdKey, generation)`
//! hashes (materials, images), see [`keys`].

pub mod camera;
/// Evaluated geometry in renders (M4, WP 4.7; stub created by M4 W0).
pub mod evaluated;
pub mod images;
pub mod keys;
pub mod lights;
pub mod mesh;
pub mod visibility;
pub mod workbench;
pub mod world;

#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::sync::Arc;

use frap_material::{ImageLookup, LoweredMaterial, TexRef};
use frap_mesh::Mesh;
use frap_nodes::NodeTree;
use frap_rna::{IdKey, IdRef, MaterialId, MeshId, ObjectId};
use frap_scene::id::{Material, Object, SlotLink};
use frap_scene::{EvalMode, EvaluatedObject, Evaluator, HostRead, Main};
use indexmap::IndexMap;

use crate::types::{
    MatKey, MeshKey, RCamera, RLight, RMesh, RObject, RSettings, RWorkbenchMaterial, RenderError,
    RenderRequest, RenderScene,
};
use camera::CameraParams;
use images::Images;

/// Builds the render scene of a (cloned, animation-applied) state.
///
/// `req.objects` restricts the drawn mesh objects (lights keep lighting the scene);
/// `req.resolution` / `req.camera` replace the scene's output size and camera; `req.frame` is
/// only reported in `settings.frame` (the caller applies animation before building).
///
/// Errors: `Unsupported("Image too small")` when the output resolution has a zero side
/// (Blender's `RE_InitState` report), `Unsupported("Cannot render, no camera")` without
/// `scene.camera` and without a camera override (the render operator reports this first,
/// §19.4), and evaluation failures.
pub fn build(
    host: &dyn HostRead,
    eval: &mut dyn Evaluator,
    req: &RenderRequest,
) -> Result<RenderScene, RenderError> {
    build_with(host, eval, req, frap_material::lower)
}

/// The material lowering [`build`] uses (`frap_material::lower`; tests substitute it).
pub(crate) type LowerFn = fn(&NodeTree, &Material, &dyn ImageLookup) -> LoweredMaterial;

/// [`build`] with an explicit material lowering.
pub(crate) fn build_with(
    host: &dyn HostRead,
    eval: &mut dyn Evaluator,
    req: &RenderRequest,
    lower: LowerFn,
) -> Result<RenderScene, RenderError> {
    let ctx = host.ctx();
    let main = host.main();
    let scene = ctx.scene();
    let view_layer = ctx.view_layer();
    let r = &scene.render;

    let res = match req.resolution {
        Some([w, h]) => [w.min(i32::MAX as u32) as i32, h.min(i32::MAX as u32) as i32],
        None => camera::render_resolution(r),
    };
    if res[0] < 1 || res[1] < 1 {
        return Err(RenderError::Unsupported("Image too small".into()));
    }

    let camera = match &req.camera {
        Some(c) => c.clone(),
        None => scene_camera(host, eval, res)?,
    };

    let mut b = Builder {
        main,
        lower,
        meshes: IndexMap::new(),
        mesh_memo: HashMap::new(),
        materials: IndexMap::new(),
        workbench_materials: IndexMap::new(),
        mat_memo: HashMap::new(),
        images: Images::default(),
    };

    let mut visible: Vec<(IdRef<ObjectId>, &Object)> =
        visibility::render_objects(main, scene, view_layer)
            .into_iter()
            .filter_map(|r| main.objects.get(r).map(|o| (r, o)))
            .collect();
    visible.sort_by(|a, b| sort_key(main, a.1).cmp(&sort_key(main, b.1)));

    let mut objects = Vec::new();
    let mut lights: Vec<RLight> = Vec::new();
    for (ob_ref, ob) in visible {
        // `--stmt-timeout` cancellation point, once per object (§12).
        frap_math::budget::check_now();
        match ob.object_type() {
            "MESH" => {
                if req
                    .objects
                    .as_ref()
                    .is_some_and(|names| !names.contains(&ob.header.name))
                {
                    continue;
                }
                let ev = eval_object(eval, host, ob_ref, ob)?;
                objects.push(b.object(host, ob, &ev));
            }
            "LIGHT" => {
                let Some(data) = ob
                    .data
                    .and_then(|d| d.typed())
                    .and_then(|l| main.lights.get(l))
                else {
                    continue;
                };
                let ev = eval_object(eval, host, ob_ref, ob)?;
                lights.push(lights::light(data, ev.matrix_world));
            }
            _ => {}
        }
    }

    let world_data = scene.world.and_then(|w| main.worlds.get(w));
    let world = world::world(world_data, &mut |im| b.images.key(main, im));
    let shading = &scene.display.shading;
    let view = &scene.view_settings;
    let settings = RSettings {
        engine: r.engine.clone(),
        taa_render_samples: scene.eevee.taa_render_samples.max(0) as u32,
        use_shadows: scene.eevee.use_shadows,
        film_transparent: r.film_transparent,
        shading_light: shading.light.clone(),
        shading_color_type: shading.color_type.clone(),
        shading_single_color: shading.single_color,
        view_transform: view.view_transform.clone(),
        look: view.look.clone(),
        exposure: view.exposure,
        gamma: view.gamma,
        frame: req.frame.unwrap_or(scene.frame_current),
        workbench_materials: b.workbench_materials,
        workbench_background: Some(workbench::background(scene, world_data)),
        workbench: workbench::shading(scene),
    };

    Ok(RenderScene {
        resolution: [res[0] as u32, res[1] as u32],
        camera,
        objects,
        meshes: b.meshes,
        materials: b.materials,
        images: b.images.map,
        lights,
        world,
        settings,
    })
}

/// `(name, library path)`: the order of `RenderScene::objects` and `lights`.
fn sort_key<'a>(main: &'a Main, ob: &'a Object) -> (&'a str, Option<&'a str>) {
    let lib = ob
        .header
        .library
        .and_then(|l| main.libraries.get(l))
        .map(|l| l.filepath.as_str());
    (ob.header.name.as_str(), lib)
}

fn eval_object(
    eval: &mut dyn Evaluator,
    host: &dyn HostRead,
    r: IdRef<ObjectId>,
    ob: &Object,
) -> Result<Arc<EvaluatedObject>, RenderError> {
    eval.object(host, r, EvalMode::Render).map_err(|e| {
        RenderError::Unsupported(format!(
            "cannot evaluate object '{}': {}",
            ob.header.name, e.message
        ))
    })
}

/// The camera of `scene.camera` (any object type, as `BKE_camera_params_from_object`).
fn scene_camera(
    host: &dyn HostRead,
    eval: &mut dyn Evaluator,
    res: [i32; 2],
) -> Result<RCamera, RenderError> {
    let ctx = host.ctx();
    let main = host.main();
    let scene = ctx.scene();
    let no_camera = || RenderError::Unsupported("Cannot render, no camera".into());
    let cam_ref = scene.camera.ok_or_else(no_camera)?;
    let ob = main.objects.get(cam_ref).ok_or_else(no_camera)?;
    let params = camera_params(main, ob);
    let ev = eval_object(eval, host, cam_ref, ob)?;
    let r = &scene.render;
    Ok(camera::render_camera(
        &params,
        &ev.matrix_world,
        res[0],
        res[1],
        r.pixel_aspect_x,
        r.pixel_aspect_y,
    ))
}

/// `BKE_camera_params_init` + `BKE_camera_params_from_object`.
pub fn camera_params(main: &Main, ob: &Object) -> CameraParams {
    let data = ob.data;
    if let Some(cam) = data
        .and_then(|d| d.typed())
        .and_then(|c| main.cameras.get(c))
    {
        return CameraParams::from_camera(&cam.fields);
    }
    if let Some(light) = data
        .and_then(|d| d.typed())
        .and_then(|l| main.lights.get(l))
    {
        return CameraParams::from_light(light);
    }
    CameraParams::default()
}

/// The material of slot `i` (`MaterialSlot.material`): the object's own material when the
/// slot links to the object, else the mesh's.
pub fn slot_material(
    ob: &Object,
    mesh_materials: &[Option<IdRef<MaterialId>>],
    i: usize,
) -> Option<IdRef<MaterialId>> {
    match ob.material_slot_overrides.get(i) {
        Some(o) if o.link == SlotLink::Object => o.material,
        _ => mesh_materials.get(i).copied().flatten(),
    }
}

struct Builder<'a> {
    main: &'a Main,
    lower: LowerFn,
    meshes: IndexMap<MeshKey, Arc<RMesh>>,
    /// `(evaluated mesh pointer, slot count)` → key; the `Arc` keeps the pointer valid.
    mesh_memo: HashMap<(usize, usize), (Arc<Mesh>, MeshKey)>,
    materials: IndexMap<MatKey, Arc<LoweredMaterial>>,
    workbench_materials: IndexMap<MatKey, RWorkbenchMaterial>,
    mat_memo: HashMap<IdKey, MatKey>,
    images: Images,
}

impl Builder<'_> {
    fn object(&mut self, host: &dyn HostRead, ob: &Object, ev: &EvaluatedObject) -> RObject {
        let main = self.main;
        let mesh_ref: Option<IdRef<MeshId>> = ob.data.and_then(|d| d.typed());
        let data = mesh_ref.and_then(|m| main.meshes.get(m));
        let evaluated = ev.mesh.clone().or_else(|| match (mesh_ref, data) {
            (Some(m), Some(_)) => Some(host.mesh_view(m)),
            _ => None,
        });
        // Slots of the evaluated object (modifiers may add some; WP 4.7, `evaluated.rs`).
        let slots = evaluated::slot_materials(ob, data, evaluated.as_deref());
        let slot_count = slots.len();
        let mesh = evaluated.map(|m| self.mesh(m, slot_count));
        let materials = slots
            .into_iter()
            .map(|m| m.and_then(|m| self.material(m)))
            .collect();
        RObject {
            name: ob.header.name.clone(),
            matrix_world: ev.matrix_world,
            mesh,
            materials,
            color: ob.color,
            random_hash: workbench::random_hash(main, ob),
            display_type: ob.display_type.clone(),
        }
    }

    fn mesh(&mut self, m: Arc<Mesh>, slot_count: usize) -> MeshKey {
        let memo = (Arc::as_ptr(&m) as usize, slot_count);
        if let Some((_, k)) = self.mesh_memo.get(&memo) {
            return *k;
        }
        let rm = mesh::extract(&m, slot_count);
        let k = keys::mesh_key(&rm);
        self.meshes.entry(k).or_insert_with(|| Arc::new(rm));
        self.mesh_memo.insert(memo, (m, k));
        k
    }

    fn material(&mut self, r: IdRef<MaterialId>) -> Option<MatKey> {
        if let Some(k) = self.mat_memo.get(&r.key()) {
            return Some(*k);
        }
        let main = self.main;
        let m = main.materials.get(r)?;
        let k = keys::mat_key(r.key(), &m.header);
        self.mat_memo.insert(r.key(), k);
        let lowered = (self.lower)(&m.node_tree, m, main);
        for t in textures(&lowered) {
            self.images.key(main, t.image);
        }
        self.materials.insert(k, Arc::new(lowered));
        self.workbench_materials.insert(k, workbench::material(m));
        Some(k)
    }
}

/// Every texture a lowered material samples.
fn textures(m: &LoweredMaterial) -> impl Iterator<Item = &TexRef> {
    [
        m.base_color.texture.as_ref(),
        m.metallic.texture.as_ref(),
        m.roughness.texture.as_ref(),
        m.normal.as_ref().map(|n| &n.texture),
        m.occlusion.as_ref(),
        m.emissive.texture.as_ref(),
    ]
    .into_iter()
    .flatten()
}
