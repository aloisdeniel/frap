// SPDX-License-Identifier: GPL-3.0-or-later
//! WP 0.2 acceptance: prototypes of `MeshVertexMut`, derived `MaterialSlot`, an embedded
//! node-tree path, `Context`, `Box<dyn RenderService>` through `OpCtx::services()` and
//! `OpCtx::split_edit`, with get/set tests through string paths. Expected strings were
//! verified on Blender 5.2.4 (see RFC 0001 appendix).

use std::sync::Arc;

use frap_probe_render_scene::{AdapterSummary, Film, GpuOptions, RenderError, RenderScene, RenderService};
use frap_probe_rna::*;
use frap_probe_scene::*;
use frap_probe_session::resolve::{GotValue, get, set};
use frap_probe_session::{Session, no_gpu};

fn value(s: &Session, path: &str) -> Value {
    match get(&s.host, path).unwrap_or_else(|e| panic!("{path}: {e:?}")).value {
        GotValue::Value(v) => v,
        other => panic!("{path}: not a value: {other:?}"),
    }
}
fn repr(s: &Session, path: &str) -> String {
    get(&s.host, path).unwrap_or_else(|e| panic!("{path}: {e:?}")).repr
}
fn set_ok(s: &mut Session, path: &str, v: Input) {
    s.transaction(|s| set(&mut s.host, path, v).map_err(FrapError::from))
        .unwrap_or_else(|e| panic!("{path}: {e:?}"));
}
fn set_err(s: &mut Session, path: &str, v: Input) -> RnaError {
    let e = s.transaction(|s| set(&mut s.host, path, v).map_err(FrapError::from)).unwrap_err();
    RnaError {
        exception: e.exception,
        code: e.code,
        message: e.message,
        hint: e.hint,
    }
}
fn tuple(v: &[f64]) -> Input {
    Input::Tuple(v.iter().map(|x| Input::Float(*x)).collect())
}
fn floats(v: &Value) -> Vec<f32> {
    v.as_f32_slice().unwrap().to_vec()
}
fn ptr_to(s: &Session, path: &str) -> Input {
    let g = get(&s.host, path).unwrap();
    match g.value {
        GotValue::Struct => Input::Ptr(g.ptr),
        _ => panic!("not a struct"),
    }
}

// ------------------------------------------------------------------------ MeshVertexMut

#[test]
fn mesh_vertex_soa_proxy_get_set() {
    let (mut s, f) = Session::factory();
    let p = "bpy.data.meshes['Cube'].vertices[3].co";
    assert_eq!(floats(&value(&s, p)), [-1.0, 1.0, 1.0]);
    let g = get(&s.host, p).unwrap();
    assert_eq!(g.repr, "bpy.data.meshes['Cube'].vertices[3].co");
    assert_eq!(g.path_from_id().as_deref(), Some("vertices[3].co"));
    assert_eq!(
        repr(&s, "bpy.data.meshes['Cube'].vertices[3]"),
        "bpy.data.meshes['Cube'].vertices[3]"
    );

    let n_before = floats(&value(&s, "bpy.data.meshes['Cube'].vertices[2].normal"));
    set_ok(&mut s, p, Input::Tuple(vec![Input::Float(0.25), Input::Int(2), Input::Float(-0.5)]));
    assert_eq!(floats(&value(&s, p)), [0.25, 2.0, -0.5]);
    assert!(s.host.log.modified.contains(&f.cube_mesh.any()));
    assert_ne!(
        floats(&value(&s, "bpy.data.meshes['Cube'].vertices[2].normal")),
        n_before,
        "cache invalidated by the proxy write"
    );

    set_ok(&mut s, "bpy.data.meshes['Cube'].vertices[3].select", Input::Bool(true));
    assert_eq!(value(&s, "bpy.data.meshes['Cube'].vertices[3].select"), Value::Bool(true));

    let e = set_err(&mut s, "bpy.data.meshes['Cube'].vertices[3].index", Input::Int(1));
    assert_eq!(e.message, "bpy_struct: attribute \"index\" from \"MeshVertex\" is read-only");
    let e = set_err(&mut s, p, tuple(&[1.0, 2.0]));
    assert_eq!(
        (e.exception, e.message.as_str()),
        (
            PyExc::ValueError,
            "bpy_struct: item.attr = val: sequences of dimension 0 should contain 3 items, not 2"
        )
    );
    let e = get(&s.host, "bpy.data.meshes['Cube'].vertices['x']").unwrap_err();
    assert_eq!(e.message, "bpy_prop_collection[key]: Mesh.vertices does not support string lookups");
    let e = get(&s.host, "bpy.data.meshes['Cube'].vertices[8]").unwrap_err();
    assert_eq!(e.message, "bpy_prop_collection[index]: index 8 out of range, size 8");
}

#[test]
fn id_item_collection_with_none() {
    let (mut s, _f) = Session::factory();
    // mesh.materials: items are IDs (re-root) and may be None (F-TR-4).
    assert_eq!(repr(&s, "bpy.data.meshes['Cube'].materials[0]"), "bpy.data.materials['Material']");
    assert_eq!(
        value(&s, "bpy.data.meshes['Cube'].materials['Material'].name"),
        Value::Str("Material".into())
    );
    let me = s.host.state.main.meshes.find("Cube").unwrap();
    s.host
        .state
        .main
        .meshes
        .get_mut(me, &mut ChangeLog::default())
        .unwrap()
        .materials
        .push(None);
    assert_eq!(
        get(&s.host, "bpy.data.meshes['Cube'].materials[1]").unwrap().value,
        GotValue::Id(None)
    );
}

// ------------------------------------------------------------------- derived MaterialSlot

#[test]
fn material_slot_derived_get_set() {
    let (mut s, f) = Session::factory();
    let ob = "bpy.data.objects['Cube']";
    assert_eq!(
        get(&s.host, &format!("{ob}.material_slots")).unwrap().value,
        GotValue::Coll { len: 1 }
    );
    // name lookup, index repr (verified)
    assert_eq!(
        repr(&s, &format!("{ob}.material_slots['Material']")),
        "bpy.data.objects['Cube'].material_slots[0]"
    );
    assert_eq!(value(&s, &format!("{ob}.material_slots[0].name")), Value::Str("Material".into()));
    assert_eq!(value(&s, &format!("{ob}.material_slots[0].link")), Value::Enum("DATA".into()));
    assert_eq!(
        get(&s.host, &format!("{ob}.material_slots[0].material")).unwrap().value,
        GotValue::Id(Some(f.material.any()))
    );

    // data.materials.append(X) touches only the mesh; the slot count follows on every user.
    let ob_gen = s.host.state.main.objects.get(f.cube).unwrap().header.generation;
    let x = {
        let (main, log) = (&mut s.host.state.main, &mut s.host.log);
        let x = main.materials.insert("X", Material::new("X"), log);
        main.meshes.get_mut(f.cube_mesh, log).unwrap().materials.push(Some(x));
        x
    };
    assert_eq!(
        get(&s.host, &format!("{ob}.material_slots")).unwrap().value,
        GotValue::Coll { len: 2 }
    );
    assert_eq!(s.host.state.main.objects.get(f.cube).unwrap().header.generation, ob_gen);

    // DATA link: slot.material = M writes mesh.materials[1] (verified), i.e. another ID.
    let m = ptr_to(&s, "bpy.data.materials['Material']");
    set_ok(&mut s, &format!("{ob}.material_slots[1].material"), m);
    let me = s.host.state.main.meshes.get(f.cube_mesh).unwrap();
    assert_eq!(me.materials[1], Some(f.material));
    assert!(s.host.log.modified.contains(&f.cube_mesh.any()) && !s.host.log.modified.contains(&f.cube.any()));

    // OBJECT link: override grows lazily; material reads the override (None), mesh untouched.
    set_ok(&mut s, &format!("{ob}.material_slots[1].link"), Input::Str("OBJECT".into()));
    assert_eq!(s.host.state.main.objects.get(f.cube).unwrap().material_slot_overrides.len(), 2);
    assert_eq!(
        get(&s.host, &format!("{ob}.material_slots[1].material")).unwrap().value,
        GotValue::Id(None)
    );
    let xp = ptr_to(&s, "bpy.data.materials['X']");
    set_ok(&mut s, &format!("{ob}.material_slots[1].material"), xp);
    assert_eq!(value(&s, &format!("{ob}.material_slots[1].name")), Value::Str("X".into()));
    assert_eq!(s.host.state.main.meshes.get(f.cube_mesh).unwrap().materials[1], Some(f.material));
    let _ = x;

    // Blender wording (verified)
    let e = set_err(&mut s, &format!("{ob}.material_slots[0].link"), Input::Str("FOO".into()));
    assert_eq!(
        e.message,
        "bpy_struct: item.attr = val: enum \"FOO\" not found in ('OBJECT', 'DATA')"
    );
    let cube = ptr_to(&s, ob);
    let e = set_err(&mut s, &format!("{ob}.material_slots[0].material"), cube);
    assert_eq!(
        e.message,
        "bpy_struct: item.attr = val: MaterialSlot.material expected a Material type, not Object"
    );
    let e = get(&s.host, &format!("{ob}.material_slots[9]")).unwrap_err();
    assert_eq!(e.message, "bpy_prop_collection[index]: index 9 out of range, size 2");
}

// -------------------------------------------------------------- embedded node-tree path

#[test]
fn embedded_node_tree_path_repr_get_set() {
    let (mut s, _f) = Session::factory();
    {
        let (main, log) = (&mut s.host.state.main, &mut s.host.log);
        main.materials.insert("X", Material::new("X"), log);
    }
    let nt = get(&s.host, "bpy.data.materials['X'].node_tree").unwrap();
    assert_eq!(nt.repr, "bpy.data.materials['X'].node_tree");
    assert!(matches!(
        nt.ptr.root,
        Root::Embedded {
            slot: EmbeddedSlot::NodeTree,
            ..
        }
    ));
    assert_eq!(value(&s, "bpy.data.materials['X'].node_tree.is_embedded_data"), Value::Bool(true));

    // Appendix A strings (verified on 5.2.4)
    let sock = "bpy.data.materials['X'].node_tree.nodes['Principled BSDF'].inputs['Base Color']";
    assert_eq!(
        repr(&s, sock),
        "bpy.data.materials['X'].node_tree.nodes[\"Principled BSDF\"].inputs[0]"
    );
    let dv = format!("{sock}.default_value");
    let g = get(&s.host, &dv).unwrap();
    assert_eq!(
        g.path_from_id().as_deref(),
        Some("nodes[\"Principled BSDF\"].inputs[0].default_value")
    );
    assert_eq!(g.ptr.srna.identifier, "NodeSocketColor");
    assert_eq!(floats(&value(&s, &dv)), [0.8, 0.8, 0.8, 1.0]);

    // Set through the owner: owner gen bump + owner logged as modified (§5.1).
    let mat = s.host.state.main.materials.find("X").unwrap();
    let gen0 = s.host.state.main.materials.get(mat).unwrap().header.generation;
    set_ok(&mut s, &dv, tuple(&[1.0, 0.0, 0.0, 1.0]));
    assert_eq!(floats(&value(&s, &dv)), [1.0, 0.0, 0.0, 1.0]);
    assert!(s.host.state.main.materials.get(mat).unwrap().header.generation > gen0);
    assert_eq!(s.host.log.modified.iter().copied().collect::<Vec<_>>(), vec![mat.any()]);

    let e = set_err(&mut s, &dv, tuple(&[1.0, 2.0, 3.0]));
    assert_eq!(
        e.message,
        "bpy_struct: item.attr = val: sequences of dimension 0 should contain 4 items, not 3"
    );
    assert_eq!(floats(&value(&s, &dv)), [1.0, 0.0, 0.0, 1.0], "rolled back");
    let e = set_err(&mut s, "bpy.data.materials['X'].node_tree", Input::None);
    assert_eq!(e.message, "bpy_struct: attribute \"node_tree\" from \"Material\" is read-only");
    let e = get(&s.host, "bpy.data.materials['X'].node_tree.nodes['Principled BSDF'].inputs['Nope']").unwrap_err();
    assert_eq!(
        (e.exception, e.message.as_str()),
        (PyExc::KeyError, "bpy_prop_collection[key]: key \"Nope\" not found")
    );
}

// ------------------------------------------------------------------------------ Context

#[test]
fn context_members_and_rna_props() {
    let (mut s, f) = Session::factory();
    assert_eq!(get(&s.host, "bpy.context.object").unwrap().value, GotValue::Id(Some(f.cube.any())));
    assert_eq!(repr(&s, "bpy.context.object.location"), "bpy.data.objects['Cube'].location");
    assert_eq!(
        get(&s.host, "bpy.context.selected_objects").unwrap().value,
        GotValue::Coll { len: 1 }
    );
    assert_eq!(value(&s, "bpy.context.selected_objects[0].name"), Value::Str("Cube".into()));
    assert_eq!(value(&s, "bpy.context.mode"), Value::Enum("OBJECT".into()));
    // pointers into an ID re-root at the canonical path (verified reprs)
    assert_eq!(repr(&s, "bpy.context.tool_settings"), "bpy.data.scenes['Scene'].tool_settings");
    assert_eq!(
        repr(&s, "bpy.context.view_layer"),
        "bpy.data.scenes['Scene'].view_layers[\"ViewLayer\"]"
    );
    assert_eq!(
        value(&s, "bpy.context.tool_settings.mesh_select_mode"),
        Value::BoolArray([true, false, false].into_iter().collect())
    );
    assert_eq!(repr(&s, "bpy.context.scene"), "bpy.context.scene");
    // writes through a context path land in the owning ID
    set_ok(&mut s, "bpy.context.object.location", tuple(&[1.0, 2.0, 3.0]));
    assert_eq!(s.host.state.main.objects.get(f.cube).unwrap().location, [1.0, 2.0, 3.0]);
    // read-only wording differs for context members vs RNA props (verified)
    let e = set_err(&mut s, "bpy.context.object", Input::None);
    assert_eq!(
        (e.exception, e.message.as_str()),
        (PyExc::AttributeError, "bpy_struct: Context property \"object\" is read-only")
    );
    let e = set_err(&mut s, "bpy.context.scene", Input::None);
    assert_eq!(e.message, "bpy_struct: attribute \"scene\" from \"Context\" is read-only");
    let e = get(&s.host, "bpy.context.foo").unwrap_err();
    assert_eq!(e.message, "'Context' object has no attribute 'foo'");
}

// --------------------------------------------- Box<dyn RenderService> via OpCtx::services()

struct WorldColourService {
    renders: usize,
}
impl RenderService for WorldColourService {
    fn render(&mut self, scene: &RenderScene) -> Result<Film, RenderError> {
        self.renders += 1;
        let [w, h] = scene.resolution;
        let c = scene.world_rgb;
        Ok(Film {
            width: w,
            height: h,
            rgba: vec![[c[0], c[1], c[2], 1.0]; (w * h) as usize],
        })
    }
    fn render_tiles(&mut self, scenes: &[RenderScene]) -> Result<Vec<Film>, RenderError> {
        scenes.iter().map(|s| self.render(s)).collect()
    }
    fn adapter_info(&self) -> AdapterSummary {
        AdapterSummary {
            name: "stub".into(),
            backend: "none".into(),
            device_type: "cpu".into(),
            driver: String::new(),
        }
    }
}
fn stub_factory(_: &GpuOptions) -> Result<Box<dyn RenderService>, RenderError> {
    Ok(Box::new(WorldColourService { renders: 0 }))
}

#[test]
fn render_service_through_opctx_services() {
    let (mut s, _f) = Session::factory_with(stub_factory);
    let (st, out) = s.invoke("frap.probe_render", vec![]).unwrap();
    assert_eq!(st, frap_probe_op::OpStatus::Finished);
    assert_eq!(out.result["width"], Value::Int(16));
    assert_eq!(out.result["objects"], Value::Int(1));
    assert_eq!(floats(&out.result["pixel0"]), [0.05, 0.05, 0.05, 1.0]);
    s.invoke("frap.probe_render", vec![]).unwrap();
    assert_eq!(s.services.factory_calls, 1, "service created lazily, once");
    assert_eq!(s.services.render.as_ref().unwrap().adapter_info().name, "stub");
}

#[test]
fn render_without_gpu_rolls_back() {
    let (mut s, _f) = Session::factory_with(no_gpu);
    let e = s.invoke("frap.probe_render", vec![]).unwrap_err();
    assert!(e.message.contains("GPU_UNAVAILABLE"), "{}", e.message);
}

#[cfg(feature = "gpu")]
#[test]
fn render_service_real_gpu_through_opctx() {
    if frap_probe_render::Gpu::new(&GpuOptions::from_env()).is_err() && std::env::var("FRAP_GPU_OPTIONAL").is_ok_and(|v| v == "1") {
        eprintln!("SKIP: no GPU adapter");
        return;
    }
    let (mut s, _f) = Session::factory_with(frap_probe_render::new_service);
    let (_, out) = s.invoke("frap.probe_render", vec![]).unwrap();
    assert_eq!(floats(&out.result["pixel0"]), [0.05, 0.05, 0.05, 1.0]);
    eprintln!("adapter: {:?}", s.services.render.as_ref().unwrap().adapter_info());
}

// ---------------------------------------------------------------------- OpCtx::split_edit

#[test]
fn split_edit_holds_context_and_edit_state() {
    let (mut s, f) = Session::factory();
    // Simulate mode_set(mode='EDIT') (handlers arrive in M2).
    s.host.state.main.objects.get_mut(f.cube, &mut ChangeLog::default()).unwrap().mode = ObjectMode::Edit;
    s.host.state.edit.meshes.insert(
        f.cube_mesh.key(),
        EditMesh {
            bm: Arc::new(BMesh::default()),
            bm_gen: 0,
        },
    );
    s.host
        .state
        .main
        .scenes
        .get_mut(f.scene, &mut ChangeLog::default())
        .unwrap()
        .tool_settings
        .mesh_select_mode = [false, true, false];
    assert_eq!(value(&s, "bpy.context.mode"), Value::Enum("EDIT_MESH".into()));
    let (_, out) = s.invoke("frap.probe_edit", vec![]).unwrap();
    assert_eq!(out.result["selected"], Value::Int(1));
    let em = &s.host.state.edit.meshes[&f.cube_mesh.key()];
    assert_eq!(em.bm.select_mode, [false, true, false]);
    assert_eq!(em.bm.verts, vec![[0.0; 3]]);
    assert!(em.bm_gen > 0);
}

// ----------------------------------------------------------- generated typed operator props

#[test]
fn primitive_cube_add_with_generated_typed_props() {
    let (mut s, _f) = Session::factory();
    let (_, out) = s
        .invoke(
            "mesh.primitive_cube_add",
            vec![
                ("size", Input::Int(4)),
                ("location", Input::Tuple(vec![Input::Int(0), Input::Int(0), Input::Int(1)])),
            ],
        )
        .unwrap();
    let Value::Ptr(p) = &out.result["object"] else { panic!() };
    let Root::Id(a) = p.root else { panic!() };
    let ob = s.host.state.main.objects.get(IdRef::from_any(a).unwrap()).unwrap();
    assert_eq!(ob.location, [0.0, 0.0, 1.0]);
    assert_eq!(
        get(&s.host, "bpy.context.selected_objects").unwrap().value,
        GotValue::Coll { len: 1 }
    );
    let me = ob.data.and_then(IdRef::<MeshId>::from_any).unwrap();
    assert_eq!(s.host.state.main.meshes.get(me).unwrap().positions()[0], [-2.0, -2.0, -2.0]);

    // §10.2 argument errors (Blender wording, Appendix A)
    let msg = |r: Result<_, FrapError>| r.map(|_| ()).unwrap_err().message;
    assert_eq!(
        msg(s.invoke("mesh.primitive_cube_add", vec![("sizee", Input::Int(2))])),
        "Converting py args to operator properties:: keyword \"sizee\" unrecognized"
    );
    assert_eq!(
        msg(s.invoke("mesh.primitive_cube_add", vec![("size", Input::Str("x".into()))])),
        "Converting py args to operator properties: MESH_OT_primitive_cube_add.size expected a float type, not str"
    );
    assert_eq!(
        msg(s.invoke("mesh.primitive_cube_add", vec![("align", Input::Str("FOO".into()))])),
        "Converting py args to operator properties: enum \"FOO\" not found in ('WORLD', 'VIEW', 'CURSOR')"
    );
    assert_eq!(
        msg(s.invoke("mesh.foo", vec![])),
        "Calling operator \"bpy.ops.mesh.foo\" error, could not be found"
    );
}

#[test]
fn registries_are_linked_and_unique() {
    let ids: Vec<_> = frap_probe_op::OPERATORS.iter().map(|d| d.spec.idname).collect();
    let mut u = ids.clone();
    u.sort();
    u.dedup();
    assert_eq!(u.len(), ids.len());
    assert!(ids.contains(&"mesh.primitive_cube_add") && ids.contains(&"frap.probe_render") && ids.contains(&"frap.probe_edit"));
    assert_eq!(RNA_HOOKS.len(), 3);
}
