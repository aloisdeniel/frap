// SPDX-License-Identifier: GPL-3.0-or-later
//! Builder tests (WP 1.15 acceptance): camera matrices vs Blender 5.2.4, deterministic object
//! order, render visibility, mesh extraction vs `corner_split`, Workbench colour sources,
//! lights, world, and building from a `State` clone without touching the original.
//!
//! The state is driven through a minimal `HostRead` and a stub `Evaluator` (world matrices from
//! loc/rot/scale and parents, or injected matrices; the original mesh), so these tests depend
//! only on `frap-scene` data and not on `frap-eval` (WP 1.13). Tests that assign materials need
//! `frap_material::lower` (WP 1.11): while it is still the W0 stub they say so and return.

use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, OnceLock};

use frap_math::EulerOrder;
use frap_math::matrix::loc_eul_o_size_to_mat4;
use frap_mesh::split::{corner_split, normal_key};
use frap_mesh::{AttrData, Attribute, Domain, Mesh};
use frap_nodes::{Link, Node, NodeTree, PropBag, Socket, SocketType, SocketValue};
use frap_rna::{CollectionId, IdAny, IdKey, IdRef, MaterialId, MeshId, ObjectId, Value};
use frap_scene::generated::plain::View3DShading;
use frap_scene::id::{
    Camera, Collection, Image, Light, Material, Object, Scene, SlotLink, SlotOverride, World,
};
use frap_scene::{
    Base, BaseFlag, ChangeLog, Context, EditState, EvalError, EvalMode, EvaluatedObject, Evaluator,
    HostRead, LayerCollection, LayerCollectionPath, Main, Preferences, ViewLayer, VirtualView3D,
    WindowState,
};
use glam::{Mat4, Vec3};
use serde_json::Value as J;

use super::camera::{self, CameraParams, GL_TO_WGPU_DEPTH};
use super::*;
use crate::types::{ImageKey, RCamera, RLightKind, RWorld, RenderRequest};

// ---------------------------------------------------------------------------------------------
// Fixture: a minimal host, a stub evaluator and a state builder.

struct TestHost {
    main: Main,
    win: WindowState,
    edit: EditState,
    prefs: Preferences,
}

impl HostRead for TestHost {
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

#[derive(Default)]
struct TestEval {
    /// Injected `matrix_world` (Blender dumps).
    overrides: HashMap<IdKey, Mat4>,
    /// Objects whose evaluation fails.
    fail: Option<IdKey>,
    calls: usize,
}

fn local_matrix(o: &Object) -> Mat4 {
    loc_eul_o_size_to_mat4(
        Vec3::from_array(o.location),
        o.rotation_euler,
        EulerOrder::XYZ,
        Vec3::from_array(o.scale),
    )
}

fn world_matrix(main: &Main, o: &Object) -> Mat4 {
    match o.parent.and_then(|p| main.objects.get(p)) {
        Some(p) => world_matrix(main, p) * o.matrix_parent_inverse * local_matrix(o),
        None => local_matrix(o),
    }
}

impl Evaluator for TestEval {
    fn object(
        &mut self,
        host: &dyn HostRead,
        ob: IdRef<ObjectId>,
        mode: EvalMode,
    ) -> Result<Arc<EvaluatedObject>, EvalError> {
        assert_eq!(mode, EvalMode::Render, "the builder evaluates for render");
        self.calls += 1;
        if self.fail == Some(ob.key()) {
            return Err(EvalError {
                message: "Dependency cycle detected".into(),
            });
        }
        let main = host.main();
        let o = main.objects.get(ob).expect("object exists");
        let matrix_world = self
            .overrides
            .get(&ob.key())
            .copied()
            .unwrap_or_else(|| world_matrix(main, o));
        let mesh = o
            .data
            .and_then(|d| d.typed::<MeshId>())
            .filter(|m| main.meshes.get(*m).is_some())
            .map(|m| host.mesh_view(m));
        Ok(Arc::new(EvaluatedObject {
            matrix_world,
            mesh,
            bounds: None,
            pose: None,
            modifier_status: vec![],
            warnings: vec![],
        }))
    }
    fn depsgraph_update(&mut self, _host: &dyn HostRead) {}
}

struct Fixture {
    host: TestHost,
    log: ChangeLog,
    /// Material lowering (`frap_material::lower` unless a test substitutes [`fake_lower`]).
    lower: LowerFn,
}

thread_local! {
    /// The image [`fake_lower`] puts on the base colour (per test thread).
    static FAKE_TEXTURE: std::cell::Cell<Option<IdRef<frap_rna::ImageId>>> =
        const { std::cell::Cell::new(None) };
}

/// A stand-in for `frap_material::lower` (WP 1.11): constant factors that differ from every
/// viewport value the tests use, plus an optional base-colour texture.
fn fake_lower(
    _tree: &NodeTree,
    mat: &Material,
    _images: &dyn frap_material::ImageLookup,
) -> frap_material::LoweredMaterial {
    use frap_material::*;
    let tex = FAKE_TEXTURE.with(|t| t.get()).map(|image| TexRef {
        image,
        uv: UvSource::ActiveRender,
        channel: ChannelSel::Rgba,
        transform: None,
        sampler: Sampler {
            filter: Filter::Linear,
            wrap_s: Wrap::Repeat,
            wrap_t: Wrap::Repeat,
        },
        colorspace_is_data: false,
    });
    LoweredMaterial {
        base_color: Channel {
            factor: [0.123, 0.456, 0.789, 1.0],
            texture: tex,
        },
        metallic: Channel {
            factor: 0.5,
            texture: None,
        },
        roughness: Channel {
            factor: 0.5,
            texture: None,
        },
        normal: None,
        occlusion: None,
        occlusion_strength: 1.0,
        emissive: Channel {
            factor: [0.0; 3],
            texture: None,
        },
        emissive_strength: 1.0,
        alpha_mode: AlphaMode::Opaque,
        double_sided: !mat.use_backface_culling,
        unlit: false,
        ext: MaterialExts::default(),
        vertex_color: None,
        diagnostics: vec![],
    }
}

impl Fixture {
    /// One scene ("Scene", view layer "ViewLayer"), nothing in it.
    fn new() -> Fixture {
        let mut log = ChangeLog::default();
        let mut main = Main::default();
        let vl = ViewLayer {
            name: "ViewLayer".into(),
            bases: vec![],
            active: None,
            layer_collection: LayerCollection::new(None),
            active_layer_collection: LayerCollectionPath::default(),
        };
        let scene = main
            .scenes
            .insert("Scene", Scene::new("", vec![vl]), &mut log);
        let win = WindowState {
            scene,
            view_layer: "ViewLayer".into(),
            view3d: VirtualView3D {
                view_matrix: Mat4::IDENTITY,
                shading: View3DShading::default(),
            },
        };
        Fixture {
            host: TestHost {
                main,
                win,
                edit: EditState::default(),
                prefs: Preferences::default(),
            },
            log,
            lower: frap_material::lower,
        }
    }

    fn main(&mut self) -> &mut Main {
        &mut self.host.main
    }

    fn scene(&mut self) -> &mut Scene {
        let s = self.host.win.scene;
        self.host
            .main
            .scenes
            .get_mut(s, &mut self.log)
            .expect("scene")
    }

    fn ob(&mut self, r: IdRef<ObjectId>) -> &mut Object {
        self.host
            .main
            .objects
            .get_mut(r, &mut self.log)
            .expect("object")
    }

    /// Adds an object linked to the master collection (or `coll`) with a base.
    fn object(
        &mut self,
        name: &str,
        data: Option<IdAny>,
        coll: Option<IdRef<CollectionId>>,
    ) -> IdRef<ObjectId> {
        let r = self
            .host
            .main
            .objects
            .insert(name, Object::new("", data), &mut self.log);
        match coll {
            None => self.scene().collection.objects.push(r),
            Some(c) => self
                .host
                .main
                .collections
                .get_mut(c, &mut self.log)
                .expect("collection")
                .objects
                .push(r),
        }
        self.scene().view_layers[0].bases.push(Base {
            object: r,
            flag: BaseFlag::empty(),
        });
        r
    }

    fn mesh_object(&mut self, name: &str, mesh: Mesh) -> IdRef<ObjectId> {
        let m = self.host.main.meshes.insert(name, mesh, &mut self.log);
        self.object(name, Some(m.any()), None)
    }

    /// A child collection of the master collection (with its layer collection).
    fn collection(&mut self, name: &str) -> IdRef<CollectionId> {
        let c = self
            .host
            .main
            .collections
            .insert(name, Collection::new(""), &mut self.log);
        let s = self.scene();
        s.collection.children.push(c);
        s.view_layers[0]
            .layer_collection
            .children
            .push(LayerCollection::new(Some(c)));
        c
    }

    fn camera(&mut self, name: &str, cam: Camera) -> IdRef<ObjectId> {
        let c = self.host.main.cameras.insert(name, cam, &mut self.log);
        let ob = self.object(name, Some(c.any()), None);
        self.scene().camera = Some(ob);
        ob
    }

    /// The factory-like camera at the factory transform.
    fn default_camera(&mut self) -> IdRef<ObjectId> {
        let ob = self.camera("Camera", Camera::new(""));
        let o = self.ob(ob);
        o.location = [7.358_891, -6.925_791, 4.958_309];
        o.rotation_euler = [1.109_319, 0.0, 0.814_928_7];
        ob
    }

    fn material(
        &mut self,
        name: &str,
        diffuse: [f32; 4],
        metallic: f32,
        roughness: f32,
    ) -> IdRef<MaterialId> {
        let mut m = Material::new("");
        m.diffuse_color = diffuse;
        m.metallic = metallic;
        m.roughness = roughness;
        self.host.main.materials.insert(name, m, &mut self.log)
    }

    fn build(&self, eval: &mut TestEval, req: &RenderRequest) -> Result<RenderScene, RenderError> {
        build_with(&self.host, eval, req, self.lower)
    }

    fn build_default(&self) -> RenderScene {
        self.build(&mut TestEval::default(), &RenderRequest::default())
            .expect("build")
    }
}

/// `frap_material::lower` is implemented (WP 1.11), not the W0 `todo!()` stub.
fn lower_available() -> bool {
    static AVAILABLE: OnceLock<bool> = OnceLock::new();
    *AVAILABLE.get_or_init(|| {
        let m = Material::new("probe");
        let main = Main::default();
        match catch_unwind(AssertUnwindSafe(|| {
            frap_material::lower(&m.node_tree, &m, &main);
        })) {
            Ok(()) => true,
            // Only the W0 `todo!()` stub skips; any other panic of the real lowering fails.
            Err(payload) => {
                let msg = payload
                    .downcast_ref::<String>()
                    .map(String::as_str)
                    .or_else(|| payload.downcast_ref::<&str>().copied())
                    .unwrap_or("");
                if msg.starts_with("not yet implemented") {
                    false
                } else {
                    std::panic::resume_unwind(payload)
                }
            }
        }
    })
}

macro_rules! require_lower {
    () => {
        if !lower_available() {
            eprintln!("skipped: frap_material::lower is the W0 stub until WP 1.11 lands");
            return;
        }
    };
}

fn cube() -> Mesh {
    frap_mesh::primitives::cube(2.0, true, [1.0; 3])
}

fn close_m4(a: &Mat4, b: &Mat4, rel: f32) -> bool {
    a.to_cols_array()
        .iter()
        .zip(b.to_cols_array().iter())
        .all(|(x, y)| (x - y).abs() <= rel * x.abs().max(y.abs()).max(1.0))
}

// ---------------------------------------------------------------------------------------------
// Camera.

const CAMERA_CASES: &str = include_str!("testdata/camera_cases.json");

fn m4(j: &J) -> Mat4 {
    let v: Vec<f32> = j
        .as_array()
        .expect("array")
        .iter()
        .map(|x| x.as_f64().expect("number") as f32)
        .collect();
    Mat4::from_cols_slice(&v)
}

fn f(j: &J) -> f32 {
    j.as_f64().expect("number") as f32
}

/// Builds the state of one dumped case; returns the fixture and the injected camera matrix.
fn camera_case(c: &J) -> (Fixture, TestEval) {
    let mut fx = Fixture::new();
    let r = &c["render"];
    {
        let s = fx.scene();
        s.render.resolution_x = r["resolution_x"].as_i64().unwrap() as i32;
        s.render.resolution_y = r["resolution_y"].as_i64().unwrap() as i32;
        s.render.resolution_percentage = r["resolution_percentage"].as_i64().unwrap() as i32;
        s.render.pixel_aspect_x = f(&r["pixel_aspect_x"]);
        s.render.pixel_aspect_y = f(&r["pixel_aspect_y"]);
    }
    let ob = match c["object_type"].as_str().unwrap() {
        "CAMERA" => {
            let j = &c["camera"];
            let mut cam = Camera::new("");
            let fl = &mut cam.fields;
            fl.r#type = j["type"].as_str().unwrap().into();
            fl.lens = f(&j["lens"]);
            fl.sensor_fit = j["sensor_fit"].as_str().unwrap().into();
            fl.sensor_width = f(&j["sensor_width"]);
            fl.sensor_height = f(&j["sensor_height"]);
            fl.shift_x = f(&j["shift_x"]);
            fl.shift_y = f(&j["shift_y"]);
            fl.clip_start = f(&j["clip_start"]);
            fl.clip_end = f(&j["clip_end"]);
            fl.ortho_scale = f(&j["ortho_scale"]);
            fx.camera("Cam", cam)
        }
        "LIGHT" => {
            let mut l = Light::new("", "SPOT");
            l.fields.spot_size = f(&c["spot_size"]);
            let lr = fx
                .main()
                .lights
                .insert("Spot", l, &mut ChangeLog::default());
            let ob = fx.object("Spot", Some(lr.any()), None);
            fx.scene().camera = Some(ob);
            ob
        }
        _ => {
            let ob = fx.object("Empty", None, None);
            fx.scene().camera = Some(ob);
            ob
        }
    };
    let mut eval = TestEval::default();
    eval.overrides.insert(ob.key(), m4(&c["matrix_world"]));
    (fx, eval)
}

#[test]
fn camera_matrices_equal_blender() {
    let doc: J = serde_json::from_str(CAMERA_CASES).expect("camera_cases.json parses");
    assert_eq!(doc["blender"].as_str(), Some("5.2.4 LTS"));
    let cases = doc["cases"].as_array().expect("cases");
    assert!(cases.len() >= 20);
    for c in cases {
        let name = c["name"].as_str().unwrap();
        let (fx, mut eval) = camera_case(c);
        let sc = fx
            .build(&mut eval, &RenderRequest::default())
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        let winx = c["winx"].as_i64().unwrap() as u32;
        let winy = c["winy"].as_i64().unwrap() as u32;
        assert_eq!(sc.resolution, [winx, winy], "{name}: resolution");

        let winmat = m4(&c["winmat"]);
        let viewmat = m4(&c["viewmat"]);
        // The port itself, in Blender's OpenGL convention.
        let params = camera_params(
            &fx.host.main,
            fx.host
                .main
                .objects
                .get(fx.host.ctx().scene().camera.unwrap())
                .unwrap(),
        );
        let r = &fx.host.ctx().scene().render;
        let ours = camera::window_matrix(
            &params,
            winx as i32,
            winy as i32,
            r.pixel_aspect_x,
            r.pixel_aspect_y,
        );
        assert!(
            close_m4(&ours, &winmat, 1e-6),
            "{name}: winmat\n frap    {:?}\n blender {:?}",
            ours.to_cols_array(),
            winmat.to_cols_array()
        );
        // What the renderer receives.
        assert!(
            close_m4(&sc.camera.projection, &(GL_TO_WGPU_DEPTH * winmat), 1e-6),
            "{name}: projection"
        );
        assert!(
            close_m4(&sc.camera.view, &viewmat, 2e-6),
            "{name}: view\n frap    {:?}\n blender {:?}",
            sc.camera.view.to_cols_array(),
            viewmat.to_cols_array()
        );
        let is_ortho = c["camera"]["type"].as_str() == Some("ORTHO");
        assert_eq!(sc.camera.is_perspective, !is_ortho, "{name}");
        assert_eq!(sc.camera.clip, [params.clip_start, params.clip_end]);
        assert_eq!(
            sc.camera.pixel_aspect,
            r.pixel_aspect_y / r.pixel_aspect_x,
            "{name}"
        );
    }
}

#[test]
fn projection_maps_clip_range_to_wgpu_depth() {
    // A point on the near plane maps to depth 0, one on the far plane to depth 1.
    for ortho in [false, true] {
        let p = CameraParams {
            is_ortho: ortho,
            ortho_scale: 6.0,
            clip_start: 0.5,
            clip_end: 40.0,
            ..CameraParams::default()
        };
        let cam = camera::render_camera(&p, &Mat4::IDENTITY, 640, 480, 1.0, 1.0);
        let depth = |z: f32| {
            let c = cam.projection * glam::Vec4::new(0.0, 0.0, -z, 1.0);
            c.z / c.w
        };
        assert!(depth(0.5).abs() < 1e-6, "ortho={ortho}");
        assert!((depth(40.0) - 1.0).abs() < 1e-5, "ortho={ortho}");
    }
}

#[test]
fn sensor_fit_rules() {
    use camera::SensorFit::*;
    assert_eq!(camera::sensor_fit(Auto, 1920.0, 1080.0), Horizontal);
    assert_eq!(camera::sensor_fit(Auto, 100.0, 100.0), Horizontal);
    assert_eq!(camera::sensor_fit(Auto, 1080.0, 1920.0), Vertical);
    assert_eq!(camera::sensor_fit(Vertical, 1920.0, 1080.0), Vertical);
    assert_eq!(camera::sensor_size(Auto, 36.0, 24.0), 36.0);
    assert_eq!(camera::sensor_size(Horizontal, 36.0, 24.0), 36.0);
    assert_eq!(camera::sensor_size(Vertical, 36.0, 24.0), 24.0);
}

#[test]
fn resolution_and_camera_errors_and_overrides() {
    let mut fx = Fixture::new();
    // No camera.
    let e = fx
        .build(&mut TestEval::default(), &RenderRequest::default())
        .unwrap_err();
    assert_eq!(
        e,
        RenderError::Unsupported("Cannot render, no camera".into())
    );
    fx.default_camera();
    // `BKE_render_resolution`: integer division.
    {
        let s = fx.scene();
        s.render.resolution_x = 1001;
        s.render.resolution_y = 777;
        s.render.resolution_percentage = 37;
    }
    assert_eq!(fx.build_default().resolution, [370, 287]);
    {
        let s = fx.scene();
        s.render.resolution_x = 4;
        s.render.resolution_percentage = 1;
    }
    let e = fx
        .build(&mut TestEval::default(), &RenderRequest::default())
        .unwrap_err();
    assert_eq!(e, RenderError::Unsupported("Image too small".into()));
    // Overrides: resolution and camera are used verbatim.
    let cam = RCamera {
        view: Mat4::from_translation(Vec3::new(1.0, 2.0, 3.0)),
        ..RCamera::default()
    };
    let req = RenderRequest {
        frame: Some(42),
        resolution: Some([64, 32]),
        camera: Some(cam.clone()),
        objects: None,
    };
    let sc = fx.build(&mut TestEval::default(), &req).unwrap();
    assert_eq!(sc.resolution, [64, 32]);
    assert_eq!(sc.camera, cam);
    assert_eq!(sc.settings.frame, 42);
    // A resolution override with the scene camera recomputes the projection for that size.
    fx.scene().render.resolution_percentage = 100;
    let req = RenderRequest {
        resolution: Some([100, 400]),
        ..RenderRequest::default()
    };
    let sc = fx.build(&mut TestEval::default(), &req).unwrap();
    let p = CameraParams::from_camera(&Camera::new("").fields);
    assert_eq!(
        sc.camera.projection,
        GL_TO_WGPU_DEPTH * camera::window_matrix(&p, 100, 400, 1.0, 1.0)
    );
}

// ---------------------------------------------------------------------------------------------
// Object order and visibility.

fn names(sc: &RenderScene) -> Vec<&str> {
    sc.objects.iter().map(|o| o.name.as_str()).collect()
}

#[test]
fn objects_sorted_by_name_and_build_is_deterministic() {
    let order_a = ["b", "Cube", "a.001", "A", "Z", "_x"];
    let order_b = ["Z", "_x", "A", "Cube", "b", "a.001"];
    let mut scenes = vec![];
    for order in [order_a, order_b] {
        let mut fx = Fixture::new();
        fx.default_camera();
        for n in order {
            let ob = fx.mesh_object(n, cube());
            // The same transform per name in both states.
            let idx = order_a.iter().position(|x| *x == n).unwrap();
            fx.ob(ob).location = [idx as f32, 0.0, 0.0];
        }
        let sc = fx.build_default();
        assert_eq!(fx.build_default(), sc, "two builds of one state are equal");
        scenes.push(sc);
    }
    // Byte order of the names (as `sort` on `str`), whatever the creation/link order.
    assert_eq!(names(&scenes[0]), ["A", "Cube", "Z", "_x", "a.001", "b"]);
    assert_eq!(names(&scenes[0]), names(&scenes[1]));
    // Same geometry → same content key, shared by every object (one `RMesh`).
    assert_eq!(scenes[0].meshes.len(), 1);
    let k = scenes[0].objects[0].mesh.unwrap();
    assert!(scenes[0].objects.iter().all(|o| o.mesh == Some(k)));
    assert_eq!(
        scenes[0].meshes.keys().collect::<Vec<_>>(),
        scenes[1].meshes.keys().collect::<Vec<_>>()
    );
    for (a, b) in scenes[0].objects.iter().zip(&scenes[1].objects) {
        assert_eq!(a.matrix_world, b.matrix_world);
        assert_eq!(a.random_hash, b.random_hash);
    }
}

#[test]
fn render_visibility_rules() {
    let mut fx = Fixture::new();
    fx.default_camera();
    let shown = fx.mesh_object("Shown", cube());
    let hidden_render = fx.mesh_object("HiddenRender", cube());
    fx.ob(hidden_render).hide_render = true;
    let hidden_viewport = fx.mesh_object("HiddenViewport", cube());
    fx.ob(hidden_viewport).hide_viewport = true;
    // `hide_set(True)` is a base flag: render ignores it.
    let base_hidden = fx.mesh_object("BaseHidden", cube());
    for b in &mut fx.scene().view_layers[0].bases {
        if b.object == base_hidden {
            b.flag |= BaseFlag::HIDDEN;
        }
    }
    // Collections: hide_render hides children too; exclude hides; hide_viewport does not.
    let c_render = fx.collection("NoRender");
    let c_child =
        fx.main()
            .collections
            .insert("Child", Collection::new(""), &mut ChangeLog::default());
    fx.main()
        .collections
        .get_mut(c_render, &mut ChangeLog::default())
        .unwrap()
        .children
        .push(c_child);
    fx.object(
        "InNoRender",
        Some(fx.host.main.meshes.find("Shown").unwrap().any()),
        Some(c_render),
    );
    fx.object(
        "InChildOfNoRender",
        Some(fx.host.main.meshes.find("Shown").unwrap().any()),
        Some(c_child),
    );
    fx.main()
        .collections
        .get_mut(c_render, &mut ChangeLog::default())
        .unwrap()
        .hide_render = true;
    let c_excl = fx.collection("Excluded");
    fx.object(
        "InExcluded",
        Some(fx.host.main.meshes.find("Shown").unwrap().any()),
        Some(c_excl),
    );
    fx.scene().view_layers[0].layer_collection.children[1].exclude = true;
    let c_vp = fx.collection("ViewportHidden");
    let in_vp = fx.object(
        "InViewportHidden",
        Some(fx.host.main.meshes.find("Shown").unwrap().any()),
        Some(c_vp),
    );
    {
        let s = fx.scene();
        let lc = &mut s.view_layers[0].layer_collection.children[2];
        lc.hide_viewport = true;
        lc.holdout = true;
        lc.indirect_only = true;
    }
    fx.main()
        .collections
        .get_mut(c_vp, &mut ChangeLog::default())
        .unwrap()
        .hide_viewport = true;
    // Linked twice: listed once.
    fx.scene().collection.objects.push(in_vp);
    // Non-geometry objects are not drawn.
    fx.object("Empty", None, None);

    let sc = fx.build_default();
    assert_eq!(
        names(&sc),
        ["BaseHidden", "HiddenViewport", "InViewportHidden", "Shown"]
    );
    let _ = shown;

    // `RenderRequest::objects` restricts the drawn objects.
    let req = RenderRequest {
        objects: Some(vec!["Shown".into(), "Missing".into()]),
        ..RenderRequest::default()
    };
    let sc = fx.build(&mut TestEval::default(), &req).unwrap();
    assert_eq!(names(&sc), ["Shown"]);
}

#[test]
fn evaluation_uses_parents_and_failures_propagate() {
    let mut fx = Fixture::new();
    fx.default_camera();
    let parent = fx.mesh_object("Parent", cube());
    fx.ob(parent).location = [1.0, 2.0, 3.0];
    let child = fx.object(
        "Child",
        Some(fx.host.main.meshes.find("Parent").unwrap().any()),
        None,
    );
    fx.ob(child).parent = Some(parent);
    fx.ob(child).location = [0.0, 0.0, 1.0];
    let sc = fx.build_default();
    let c = sc.objects.iter().find(|o| o.name == "Child").unwrap();
    assert_eq!(c.matrix_world.w_axis.truncate(), Vec3::new(1.0, 2.0, 4.0));

    let mut eval = TestEval {
        fail: Some(child.key()),
        ..TestEval::default()
    };
    let e = fx.build(&mut eval, &RenderRequest::default()).unwrap_err();
    assert_eq!(
        e,
        RenderError::Unsupported(
            "cannot evaluate object 'Child': Dependency cycle detected".into()
        )
    );
}

// ---------------------------------------------------------------------------------------------
// Mesh extraction.

/// Every triangle corner of `rm` carries the data of the mesh corner it came from, and the
/// split vertices are exactly `corner_split`'s.
fn assert_extraction_equals_corner_split(mesh: &Mesh, slot_count: usize) {
    let rm = mesh::extract(mesh, slot_count);
    let split = corner_split(mesh);
    assert!(rm.positions.len() >= split.len());
    let pos = mesh.positions();
    let cn = mesh.corner_normals();
    let cv = mesh.corner_verts();
    for i in 0..split.len() {
        assert_eq!(rm.positions[i], pos[split.vert_source[i] as usize]);
        assert_eq!(rm.normals[i], cn[split.vert_corner[i] as usize]);
    }
    // Triangles: loop triangles, mapped through `corner_vert`, grouped by slot.
    let looptris = mesh.looptris();
    assert_eq!(rm.indices.len(), looptris.len() * 3);
    let mut seen: Vec<[u32; 3]> = rm.indices.chunks(3).map(|t| [t[0], t[1], t[2]]).collect();
    let mut expected: Vec<[u32; 3]> = looptris
        .iter()
        .map(|t| t.map(|c| split.corner_vert[c as usize]))
        .collect();
    seen.sort_unstable();
    expected.sort_unstable();
    assert_eq!(seen, expected);
    for tri in looptris.iter() {
        for &c in tri {
            let v = split.corner_vert[c as usize] as usize;
            assert_eq!(rm.positions[v], pos[cv[c as usize] as usize]);
            assert_eq!(normal_key(rm.normals[v]), normal_key(cn[c as usize]));
            for (n, (_, uv)) in mesh.uv_maps().iter().enumerate() {
                assert_eq!(rm.uvs[n][v], uv[c as usize]);
            }
        }
    }
    // Submeshes tile the index buffer in ascending slot order.
    let mut next = 0;
    let mut last_slot = None;
    for (slot, r) in &rm.submeshes {
        assert_eq!(r.start, next);
        assert!(r.end > r.start);
        assert!(last_slot < Some(*slot));
        assert!((*slot as usize) < slot_count.max(1));
        last_slot = Some(*slot);
        next = r.end;
    }
    assert_eq!(next as usize, rm.indices.len());
    // Edges: one per mesh edge, between vertices at the edge's positions.
    assert_eq!(rm.edges.len(), mesh.edges_num());
    for (e, re) in mesh.edges().iter().zip(&rm.edges) {
        assert_eq!(rm.positions[re[0] as usize], pos[e[0] as usize]);
        assert_eq!(rm.positions[re[1] as usize], pos[e[1] as usize]);
    }
}

#[test]
fn mesh_extraction_equals_corner_split() {
    use frap_mesh::primitives::{cylinder, ico_sphere, monkey, uv_sphere};
    let c = cube();
    let rm = mesh::extract(&c, 0);
    // Default cube: 24 split vertices, 12 triangles, one submesh on slot 0.
    assert_eq!(rm.positions.len(), 24);
    assert_eq!(rm.indices.len(), 36);
    assert_eq!(rm.submeshes, vec![(0, 0..36)]);
    assert_eq!(rm.uvs.len(), 1);
    assert_extraction_equals_corner_split(&c, 0);

    let mut smooth = uv_sphere::uv_sphere(&uv_sphere::UvSphere::default());
    assert_extraction_equals_corner_split(&smooth, 1);
    smooth.shade_smooth();
    assert_extraction_equals_corner_split(&smooth, 1);
    assert_extraction_equals_corner_split(&monkey::monkey(&monkey::Monkey::default()), 1);
    assert_extraction_equals_corner_split(&cylinder::cylinder(&cylinder::Cylinder::default()), 2);
    assert_extraction_equals_corner_split(
        &ico_sphere::ico_sphere(&ico_sphere::IcoSphere::default()),
        0,
    );
}

#[test]
fn material_index_is_clamped_and_bucketed() {
    let mut c = cube();
    c.attributes_mut().insert(Attribute {
        name: mesh::MATERIAL_INDEX.into(),
        domain: Domain::Face,
        data: AttrData::Int(Arc::new(vec![0, 2, 1, 7, -3, 1])),
    });
    // Three slots: 7 clamps to 2, -3 to 0.
    let rm = mesh::extract(&c, 3);
    assert_eq!(rm.submeshes, vec![(0, 0..12), (1, 12..24), (2, 24..36)]);
    assert_extraction_equals_corner_split(&c, 3);
    // No slots: everything on slot 0.
    let rm = mesh::extract(&c, 0);
    assert_eq!(rm.submeshes, vec![(0, 0..36)]);
}

#[test]
fn loose_geometry_and_colours() {
    let mut m = Mesh::new("Loose");
    m.from_pydata(
        &[
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [5.0, 5.0, 5.0],
            [6.0, 5.0, 5.0],
        ],
        &[[3, 4]],
        &[vec![0, 1, 2]],
        false,
    );
    m.attributes_mut().insert(Attribute {
        name: "Col".into(),
        domain: Domain::Point,
        data: AttrData::ByteColor(Arc::new(vec![
            [255, 128, 0, 255],
            [255, 128, 0, 255],
            [255, 128, 0, 255],
            [0, 255, 0, 255],
            [0, 0, 255, 128],
        ])),
    });
    m.attributes_mut().insert(Attribute {
        name: "Lin".into(),
        domain: Domain::Corner,
        data: AttrData::FloatColor(Arc::new(vec![
            [0.1, 0.2, 0.3, 1.0],
            [0.4, 0.5, 0.6, 1.0],
            [0.7, 0.8, 0.9, 0.5],
        ])),
    });
    let rm = mesh::extract(&m, 0);
    // 3 face vertices, then the 2 loose ones.
    assert_eq!(rm.positions.len(), 5);
    assert_eq!(rm.positions[3], [5.0, 5.0, 5.0]);
    assert_eq!(rm.positions[4], [6.0, 5.0, 5.0]);
    assert_eq!(rm.indices, vec![0, 1, 2]);
    assert!(rm.edges.contains(&[3, 4]));
    assert_eq!(rm.colors.len(), 2);
    let srgb = frap_color::spaces::srgb_to_linear(128.0 / 255.0);
    assert_eq!(rm.colors[0][0], [1.0, srgb, 0.0, 1.0]);
    assert_eq!(rm.colors[1][2], [0.7, 0.8, 0.9, 0.5]);
    assert!(rm.colors.iter().all(|c| c.len() == 5));
    // Loose vertices keep their POINT colour; corner colours have no value there.
    assert_eq!(rm.colors[0][3], [0.0, 1.0, 0.0, 1.0]);
    assert_eq!(rm.colors[0][4], [0.0, 0.0, 1.0, 128.0 / 255.0]);
    assert_eq!(rm.colors[1][3], [0.0, 0.0, 0.0, 1.0]);
    assert_eq!(rm.colors[1][4], [0.0, 0.0, 0.0, 1.0]);
    assert_extraction_equals_corner_split(&m, 0);
}

// ---------------------------------------------------------------------------------------------
// Workbench colour sources.

#[test]
fn random_hash_matches_blender() {
    // `random_hash` of the Blender-rendered fixture of crates/frap-render/tests/workbench_ref.rs
    // (make_workbench_ref.py, Blender 5.2.4).
    let mut fx = Fixture::new();
    for (name, hash) in [
        ("Cube", 2_579_451_210u32),
        ("Plane", 3_619_425_818),
        ("Sphere", 512_523_784),
    ] {
        let ob = fx.object(name, None, None);
        let o = fx.host.main.objects.get(ob).unwrap();
        assert_eq!(workbench::random_hash(&fx.host.main, o), hash, "{name}");
    }
    assert_eq!(workbench::strhash_murmur(""), workbench::murmur2(&[0]));
}

#[test]
fn workbench_background_rules() {
    // Verified on 5.2.4 (Workbench renders with each combination).
    let mut fx = Fixture::new();
    let w = fx
        .main()
        .worlds
        .insert("World", World::new(""), &mut ChangeLog::default());
    fx.main()
        .worlds
        .get_mut(w, &mut ChangeLog::default())
        .unwrap()
        .color = [0.1, 0.2, 0.3];
    fx.scene().display.shading.background_color = [0.9, 0.1, 0.1];
    let world = fx.host.main.worlds.get(w).cloned();
    let s = fx.scene().clone();
    assert_eq!(s.display.shading.background_type, "THEME");
    assert_eq!(
        workbench::background(&s, world.as_ref()),
        [0.1, 0.2, 0.3, 1.0]
    );
    assert_eq!(workbench::background(&s, None), [0.0; 4]);
    let mut v = s.clone();
    v.display.shading.background_type = "WORLD".into();
    assert_eq!(
        workbench::background(&v, world.as_ref()),
        [0.1, 0.2, 0.3, 1.0]
    );
    v.display.shading.background_type = "VIEWPORT".into();
    assert_eq!(
        workbench::background(&v, world.as_ref()),
        [0.9, 0.1, 0.1, 1.0]
    );
    assert_eq!(workbench::background(&v, None), [0.9, 0.1, 0.1, 1.0]);
    v.render.film_transparent = true;
    assert_eq!(workbench::background(&v, world.as_ref()), [0.0; 4]);

    // Through the builder.
    fx.default_camera();
    assert_eq!(
        fx.build_default().settings.workbench_background,
        Some([0.0; 4])
    );
    fx.scene().world = Some(w);
    assert_eq!(
        fx.build_default().settings.workbench_background,
        Some([0.1, 0.2, 0.3, 1.0])
    );
}

#[test]
fn workbench_object_and_settings_sources() {
    let mut fx = Fixture::new();
    fx.default_camera();
    let ob = fx.mesh_object("Cube", cube());
    fx.ob(ob).color = [0.2, 0.4, 0.6, 0.5];
    {
        let s = fx.scene();
        s.display.shading.light = "FLAT".into();
        s.display.shading.color_type = "OBJECT".into();
        s.display.shading.single_color = [0.1, 0.2, 0.3];
        s.render.engine = "BLENDER_WORKBENCH".into();
        s.render.film_transparent = true;
        s.eevee.taa_render_samples = 16;
        s.view_settings.view_transform = "Standard".into();
        s.view_settings.exposure = 0.5;
        s.frame_current = 12;
    }
    let sc = fx.build_default();
    let o = &sc.objects[0];
    assert_eq!(o.color, [0.2, 0.4, 0.6, 0.5]);
    assert_eq!(o.random_hash, 2_579_451_210);
    // A mesh without materials has no slots: the renderer uses Blender's default material.
    assert!(o.materials.is_empty());
    let st = &sc.settings;
    assert_eq!(
        (st.shading_light.as_str(), st.shading_color_type.as_str()),
        ("FLAT", "OBJECT")
    );
    assert_eq!(st.shading_single_color, [0.1, 0.2, 0.3]);
    assert_eq!(st.engine, "BLENDER_WORKBENCH");
    assert!(st.film_transparent);
    assert_eq!(st.taa_render_samples, 16);
    assert_eq!(st.view_transform, "Standard");
    assert_eq!(st.exposure, 0.5);
    assert_eq!(st.frame, 12);
    assert_eq!(st.workbench_background, Some([0.0; 4]));
}

#[test]
fn workbench_material_sources() {
    let mut fx = Fixture::new();
    let m = Material::new("x");
    assert_eq!(
        workbench::material(&m),
        crate::types::RWorkbenchMaterial {
            diffuse_color: [0.8, 0.8, 0.8, 1.0],
            metallic: 0.0,
            roughness: 0.4
        }
    );
    // Slot resolution (no lowering needed).
    let steel = fx.material("Steel", [0.6, 0.6, 0.7, 1.0], 1.0, 0.3);
    let red = fx.material("Red", [1.0, 0.0, 0.0, 1.0], 0.0, 0.5);
    let mut ob = Object::new("o", None);
    let slots = [Some(steel), None, Some(steel)];
    ob.material_slot_overrides = vec![
        SlotOverride::default(),
        SlotOverride {
            link: SlotLink::Object,
            material: Some(red),
        },
        SlotOverride {
            link: SlotLink::Object,
            material: None,
        },
    ];
    // An empty OBJECT-linked slot falls back to the data (`BKE_object_material_get_eval`).
    let mut slot_mesh = Mesh::new("");
    slot_mesh.materials = slots.to_vec();
    assert_eq!(
        super::evaluated::slot_materials(&ob, Some(&slot_mesh), None),
        [Some(steel), Some(red), Some(steel)]
    );

    // Through the builder (lowering stubbed: these values come from the material itself).
    fx.lower = fake_lower;
    fx.default_camera();
    let tex = fx
        .main()
        .images
        .insert("Tex", Image::new("", 2, 2), &mut ChangeLog::default());
    FAKE_TEXTURE.with(|t| t.set(Some(tex)));
    let mut mesh = cube();
    mesh.materials = vec![Some(steel), None, Some(red)];
    let cube_ob = fx.mesh_object("Cube", mesh);
    fx.ob(cube_ob).material_slot_overrides = vec![
        SlotOverride::default(),
        SlotOverride {
            link: SlotLink::Object,
            material: Some(red),
        },
    ];
    let sc = fx.build_default();
    let o = &sc.objects[0];
    assert_eq!(o.materials.len(), 3);
    let k_steel = o.materials[0].unwrap();
    let k_red = o.materials[1].unwrap();
    assert_eq!(o.materials[2], Some(k_red));
    // One lowered material and one viewport entry per material, in object-slot order.
    assert_eq!(sc.materials.keys().collect::<Vec<_>>(), [&k_steel, &k_red]);
    let wb = &sc.settings.workbench_materials;
    assert_eq!(wb.keys().collect::<Vec<_>>(), [&k_steel, &k_red]);
    assert_eq!(wb[&k_steel].diffuse_color, [0.6, 0.6, 0.7, 1.0]);
    assert_eq!((wb[&k_steel].metallic, wb[&k_steel].roughness), (1.0, 0.3));
    assert_eq!(wb[&k_red].diffuse_color, [1.0, 0.0, 0.0, 1.0]);
    assert_eq!(
        sc.materials[&k_steel].base_color.factor,
        [0.123, 0.456, 0.789, 1.0]
    );
    // The texture's image is decoded once (generated: its colour) and keyed.
    assert_eq!(sc.images.len(), 1);
    let img = sc.images.values().next().unwrap();
    assert_eq!((img.width, img.height), (2, 2));
    // Any mutation of the material (generation bump) gives a new key; others keep theirs.
    fx.main()
        .materials
        .get_mut(steel, &mut ChangeLog::default())
        .unwrap()
        .diffuse_color = [0.0, 1.0, 0.0, 1.0];
    let sc2 = fx.build_default();
    let k_steel2 = sc2.objects[0].materials[0].unwrap();
    assert_ne!(k_steel2, k_steel);
    assert_eq!(sc2.objects[0].materials[1], Some(k_red));
    assert_eq!(
        sc2.settings.workbench_materials[&k_steel2].diffuse_color,
        [0.0, 1.0, 0.0, 1.0]
    );
    FAKE_TEXTURE.with(|t| t.set(None));
}

#[test]
fn real_lowering_feeds_render_materials() {
    require_lower!();
    let mut fx = Fixture::new();
    fx.default_camera();
    let steel = fx.material("Steel", [0.6, 0.6, 0.7, 1.0], 1.0, 0.3);
    let mut mesh = cube();
    mesh.materials = vec![Some(steel)];
    fx.mesh_object("Cube", mesh);
    let sc = fx.build_default();
    let k = sc.objects[0].materials[0].unwrap();
    let m = fx.host.main.materials.get(steel).unwrap();
    assert_eq!(
        *sc.materials[&k],
        frap_material::lower(&m.node_tree, m, &fx.host.main)
    );
    assert_eq!(
        sc.settings.workbench_materials[&k].diffuse_color,
        [0.6, 0.6, 0.7, 1.0]
    );
}

// ---------------------------------------------------------------------------------------------
// Lights and world.

#[test]
fn lights_keep_blender_units() {
    let mut fx = Fixture::new();
    fx.default_camera();
    let mut log = ChangeLog::default();
    let mut add = |fx: &mut Fixture, name: &str, ty: &str, edit: &dyn Fn(&mut Light)| {
        let mut l = Light::new("", ty);
        edit(&mut l);
        let lr = fx.main().lights.insert(name, l, &mut log);
        fx.object(name, Some(lr.any()), None)
    };
    let p = add(&mut fx, "Point", "POINT", &|l| {
        l.fields.energy = 1000.0;
        l.fields.shadow_soft_size = 0.1;
        l.fields.color = [1.0, 0.5, 0.25];
    });
    fx.ob(p).location = [4.0, 1.0, 6.0];
    add(&mut fx, "Sun", "SUN", &|l| {
        l.fields.energy = 3.0;
        l.fields.angle = 0.05;
        l.fields.exposure = 1.0;
    });
    add(&mut fx, "Spot", "SPOT", &|l| {
        l.fields.spot_size = 0.8;
        l.fields.spot_blend = 0.3;
        l.fields.use_shadow = false;
    });
    add(&mut fx, "Area", "AREA", &|l| {
        l.fields.shape = "RECTANGLE".into();
        l.fields.size = 2.0;
        l.fields.size_y = 0.5;
    });
    let hidden = add(&mut fx, "Hidden", "POINT", &|_| {});
    fx.ob(hidden).hide_render = true;
    let sc = fx.build_default();
    assert!(sc.objects.is_empty());
    assert_eq!(sc.lights.len(), 4);
    // Sorted by object name: Area, Point, Spot, Sun.
    assert!(
        matches!(&sc.lights[0].kind, RLightKind::Area { shape, size } if shape == "RECTANGLE" && *size == [2.0, 0.5])
    );
    let pt = &sc.lights[1];
    assert_eq!(pt.kind, RLightKind::Point);
    assert_eq!(
        (pt.energy, pt.radius, pt.color),
        (1000.0, 0.1, [1.0, 0.5, 0.25])
    );
    assert_eq!(pt.matrix_world.w_axis.truncate(), Vec3::new(4.0, 1.0, 6.0));
    assert_eq!(
        sc.lights[2].kind,
        RLightKind::Spot {
            size: 0.8,
            blend: 0.3
        }
    );
    assert!(!sc.lights[2].use_shadow);
    assert_eq!(sc.lights[3].kind, RLightKind::Sun { angle: 0.05 });
    assert_eq!(sc.lights[3].energy, 6.0, "exposure 1 doubles the power");
}

fn socket(name: &str, ty: SocketType, v: SocketValue) -> Socket {
    Socket {
        name: name.into(),
        identifier: name.into(),
        ty,
        enabled: true,
        hide_value: false,
        default_value: v,
    }
}

fn node(uid: u32, bl_idname: &'static str, inputs: Vec<Socket>, outputs: Vec<Socket>) -> Node {
    Node {
        uid,
        name: bl_idname.into(),
        label: String::new(),
        bl_idname,
        location: [0.0, 0.0],
        width: 140.0,
        parent: None,
        mute: false,
        hide: false,
        inputs,
        outputs,
        props: PropBag::default(),
    }
}

fn link(from: u32, from_socket: &str, to: u32, to_socket: &str) -> Link {
    Link {
        from_node: from,
        from_socket: from_socket.into(),
        to_node: to,
        to_socket: to_socket.into(),
    }
}

/// Background (colour, strength) → World Output, as `worlds.new()` builds it.
fn background_tree(color: [f32; 4], strength: f32) -> NodeTree {
    let mut t = NodeTree::new("Shader Nodetree", frap_nodes::NodeTreeType::Shader, true);
    let mut out = node(
        0,
        "ShaderNodeOutputWorld",
        vec![socket("Surface", SocketType::Shader, SocketValue::None)],
        vec![],
    );
    out.props
        .values
        .insert("is_active_output", Value::Bool(true));
    out.props.values.insert("target", Value::Enum("ALL".into()));
    let bg = node(
        1,
        "ShaderNodeBackground",
        vec![
            socket("Color", SocketType::Rgba, SocketValue::Color(color)),
            socket("Strength", SocketType::Value, SocketValue::Float(strength)),
        ],
        vec![socket("Background", SocketType::Shader, SocketValue::None)],
    );
    t.nodes = vec![out, bg];
    t.links = vec![link(1, "Background", 0, "Surface")];
    t.next_node_uid = 2;
    t
}

#[test]
fn world_from_node_tree() {
    let mut no_image = |_: IdRef<frap_rna::ImageId>| -> ImageKey { panic!("no image expected") };
    // No world: black.
    assert_eq!(
        world::world(None, &mut no_image),
        RWorld::Color {
            rgb: [0.0; 3],
            strength: 1.0
        }
    );
    // Empty tree: `World.color`.
    let mut w = World::new("w");
    w.color = [0.2, 0.3, 0.4];
    assert_eq!(
        world::world(Some(&w), &mut no_image),
        RWorld::Color {
            rgb: [0.2, 0.3, 0.4],
            strength: 1.0
        }
    );
    // Background node.
    w.node_tree = background_tree([0.5, 0.6, 0.7, 1.0], 2.5);
    assert_eq!(
        world::world(Some(&w), &mut no_image),
        RWorld::Color {
            rgb: [0.5, 0.6, 0.7],
            strength: 2.5
        }
    );
    // An EEVEE-target output wins over an ALL one, even when that one is active.
    let mut eevee = node(
        2,
        "ShaderNodeOutputWorld",
        vec![socket("Surface", SocketType::Shader, SocketValue::None)],
        vec![],
    );
    eevee
        .props
        .values
        .insert("target", Value::Enum("EEVEE".into()));
    let bg2 = node(
        3,
        "ShaderNodeBackground",
        vec![
            socket(
                "Color",
                SocketType::Rgba,
                SocketValue::Color([1.0, 0.0, 0.0, 1.0]),
            ),
            socket("Strength", SocketType::Value, SocketValue::Float(1.0)),
        ],
        vec![socket("Background", SocketType::Shader, SocketValue::None)],
    );
    w.node_tree.nodes.extend([eevee, bg2]);
    w.node_tree.links.push(link(3, "Background", 2, "Surface"));
    assert_eq!(
        world::world(Some(&w), &mut no_image),
        RWorld::Color {
            rgb: [1.0, 0.0, 0.0],
            strength: 1.0
        }
    );
    // A Cycles-only output is ignored.
    w.node_tree.nodes[2]
        .props
        .values
        .insert("target", Value::Enum("CYCLES".into()));
    assert!(
        matches!(world::world(Some(&w), &mut no_image), RWorld::Color { strength, .. } if strength == 2.5)
    );
}

#[test]
fn hdri_world_and_missing_images() {
    let mut fx = Fixture::new();
    fx.default_camera();
    let mut im = Image::new("", 0, 0);
    im.source = "FILE".into();
    im.filepath = "//does/not/exist.hdr".into();
    let img = fx
        .main()
        .images
        .insert("Sky", im, &mut ChangeLog::default());
    let mut w = World::new("");
    w.node_tree = background_tree([1.0; 4], 0.75);
    let mut env = node(
        2,
        "ShaderNodeTexEnvironment",
        vec![socket(
            "Vector",
            SocketType::Vector,
            SocketValue::Vector([0.0; 4]),
        )],
        vec![socket(
            "Color",
            SocketType::Rgba,
            SocketValue::Color([0.0; 4]),
        )],
    );
    env.props.values.insert("image", Value::Id(Some(img.any())));
    let mapping = node(
        3,
        "ShaderNodeMapping",
        vec![socket(
            "Rotation",
            SocketType::Vector,
            SocketValue::Vector([0.0, 0.0, 1.25, 0.0]),
        )],
        vec![socket(
            "Vector",
            SocketType::Vector,
            SocketValue::Vector([0.0; 4]),
        )],
    );
    w.node_tree.nodes.extend([env, mapping]);
    w.node_tree.links.push(link(2, "Color", 1, "Color"));
    w.node_tree.links.push(link(3, "Vector", 2, "Vector"));
    let wr = fx
        .main()
        .worlds
        .insert("World", w, &mut ChangeLog::default());
    fx.scene().world = Some(wr);
    let sc = fx.build_default();
    let RWorld::Hdri {
        image,
        rotation,
        strength,
    } = sc.world
    else {
        panic!("expected an HDRI world, got {:?}", sc.world);
    };
    assert_eq!((rotation, strength), (1.25, 0.75));
    // The file is missing: Blender's magenta.
    assert_eq!(*sc.images[&image], images::missing());
    assert_eq!(sc.images.len(), 1);
    // Paths: `//` is relative to the .frap's directory.
    assert_eq!(
        images::resolve_path("//tex/a.png", Some("/p/scene.frap")),
        std::path::PathBuf::from("/p/tex/a.png")
    );
    assert_eq!(
        images::resolve_path("tex/a.png", Some("/p/scene.frap")),
        std::path::PathBuf::from("tex/a.png")
    );
    // A packed PNG decodes; a generated image is filled with its colour.
    let mut gen_im = Image::new("", 2, 3);
    gen_im.generated_color = [0.25, 0.5, 0.75, 1.0];
    let d = images::decode(&gen_im, None).unwrap();
    assert_eq!((d.width, d.height, d.channels), (2, 3, 4));
    assert_eq!(
        d.pixels,
        frap_imbuf::PixelData::F32([0.25, 0.5, 0.75, 1.0].repeat(6))
    );
    let bytes = frap_imbuf::encode(
        &d,
        &frap_imbuf::EncodeFormat::Png {
            bit_depth: 8,
            compression: 15,
        },
    )
    .expect("PNG encode");
    let mut packed = Image::new("", 0, 0);
    packed.source = "FILE".into();
    packed.packed_file = Some(Arc::new(frap_scene::id::EncodedImage {
        bytes,
        mime: "image/png".into(),
    }));
    let p = images::decode(&packed, None).expect("packed PNG decodes");
    assert_eq!((p.width, p.height), (2, 3));
}

// ---------------------------------------------------------------------------------------------
// Building from a state clone.

#[test]
fn builds_from_a_state_clone_without_touching_the_session_state() {
    let mut fx = Fixture::new();
    fx.default_camera();
    let a = fx.mesh_object("A", cube());
    fx.ob(a).location = [1.0, 0.0, 0.0];
    let mut l = Light::new("", "POINT");
    l.fields.energy = 100.0;
    let lr = fx
        .main()
        .lights
        .insert("Lamp", l, &mut ChangeLog::default());
    fx.object("Lamp", Some(lr.any()), None);
    let w = fx
        .main()
        .worlds
        .insert("World", World::new(""), &mut ChangeLog::default());
    fx.scene().world = Some(w);

    // The session state and its COW clone (what `Host::snapshot_at` hands the render op).
    let session = &fx.host;
    let snapshot = TestHost {
        main: session.main.clone(),
        win: session.win.clone(),
        edit: session.edit.clone(),
        prefs: session.prefs.clone(),
    };
    let generations = |m: &Main| -> Vec<(String, u64)> {
        let mut v = vec![];
        for attr in Main::COLLECTIONS {
            for id in m.collection(attr).unwrap().0 {
                let h = m.header(id).unwrap();
                v.push((format!("{attr}/{}", h.name), h.generation));
            }
        }
        v
    };
    let before = generations(&session.main);
    let mut eval = TestEval::default();
    let sc = build(&snapshot, &mut eval, &RenderRequest::default()).unwrap();
    assert_eq!(sc.objects.len(), 1);
    assert_eq!(sc.lights.len(), 1);
    assert!(eval.calls >= 3, "camera, mesh object and light evaluated");

    // Nothing in the session changed (no generation bump, nothing added or removed) …
    assert_eq!(generations(&session.main), before);
    assert_eq!(generations(&snapshot.main), before);
    // … and the clone still shares every datablock with it: the build never wrote through
    // `Arc::make_mut`.
    for (r, _) in session.main.objects.iter() {
        assert!(Arc::ptr_eq(
            &session.main.objects.get_arc(r).unwrap(),
            &snapshot.main.objects.get_arc(r).unwrap()
        ));
    }
    for (r, _) in session.main.meshes.iter() {
        assert!(Arc::ptr_eq(
            &session.main.meshes.get_arc(r).unwrap(),
            &snapshot.main.meshes.get_arc(r).unwrap()
        ));
    }
    let s = session.win.scene;
    assert!(Arc::ptr_eq(
        &session.main.scenes.get_arc(s).unwrap(),
        &snapshot.main.scenes.get_arc(s).unwrap()
    ));
    // Building from the original gives the same scene.
    assert_eq!(
        build(session, &mut TestEval::default(), &RenderRequest::default()).unwrap(),
        sc
    );
}

#[test]
fn camera_params_of_other_object_types() {
    let mut fx = Fixture::new();
    let e = fx.object("Empty", None, None);
    let p = camera_params(&fx.host.main, fx.host.main.objects.get(e).unwrap());
    assert_eq!(p, CameraParams::default());
    assert_eq!((p.lens, p.clip_start, p.clip_end), (35.0, 0.1, 100.0));
    let mut l = Light::new("", "SPOT");
    l.fields.spot_size = std::f32::consts::FRAC_PI_2;
    let p = CameraParams::from_light(&l);
    // spot_size 90°: lens = 16 cos(45°) / sin(45°) = 16.
    assert!((p.lens - 16.0).abs() < 1e-5, "{}", p.lens);
}
