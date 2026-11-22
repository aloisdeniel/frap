// SPDX-License-Identifier: GPL-3.0-or-later
//! Every CONTRACT signature of ARCHITECTURE.md (§3.4, frozen at M1 W0, as amended by RFC 0001)
//! compiles with the documented types. Each item is pinned by a typed function-pointer or a
//! typed binding, so a signature change breaks this file, not a downstream WP. Bodies that are
//! still `todo!()` are never called here. `Response`/serve framing and `GoldenState` (L11/L12)
//! are pinned in `crates/frap-golden/tests/contracts.rs` (frap-ops-all cannot dev-depend on
//! frap-session: rule 2.2.11).
#![allow(dead_code, clippy::type_complexity, clippy::let_unit_value)]

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;
use std::sync::Arc;

use frap_anim::{Action, AnimData};
use frap_bmesh::BMesh;
use frap_lang::{
    Arg, AssignOp, BinOp, Builtin, Command, Expr, Form, Lit, NoopReason, OpMetaKind, PathExpr,
    PathRoot, PathSeg, Stmt, UnOp,
};
use frap_material::{
    AlphaMode, Channel, ImageLookup, LoweredMaterial, MaterialExts, NameAlloc, NormalMap, TexRef,
    VertexColorUse,
};
use frap_mesh::{AttrData, Attribute, AttributeSet, DeformVert, Domain, Mesh};
use frap_nodes::{Link, Node, NodeTree, NodeTreeType, Socket};
use frap_op::{
    DynEnumFn, ImageCache, MODE_HANDLERS, ModeHandler, OPERATORS, OpCtx, OpError, OpKind, OpOutput,
    OpResult, OpStatus, OperatorDef, PathResolver, Placement, Poll, PollFail, PollReq, Services,
    Support, TRANSFORM_MODE_HANDLERS, Tier, TransformModeHandler, TransformOp,
};
use frap_render_scene::{
    AdapterSummary, Film, GpuOptions, RMesh, RenderError, RenderRequest, RenderScene, RenderService,
};
use frap_rna::*;
use frap_scene::id::{Material, Object};
use frap_scene::{
    ChangeLog, Context, ContextMode, EditMesh, EditState, EvalError, EvalMode, EvaluatedObject,
    Evaluator, FuncArgs, Host, HostRead, IdMap, Main, RNA_FUNCS, RNA_HOOKS, RnaFuncImpl,
    RnaFuncReg, RnaGetHook, RnaHookReg, RnaSetHook, Snapshot, ViewLayer, WindowState,
};
use indexmap::IndexMap;
use smallvec::SmallVec;
use smol_str::SmolStr;

// ---------------------------------------------------------------- §5.1 identity

fn identity() {
    let _: fn() -> IdKey = IdKey::next;
    let _: fn(IdCode) -> &'static str = IdCode::collection;
    let _: fn(IdKey) -> IdRef<MaterialId> = IdRef::<MaterialId>::from_key;
    let _: fn(IdRef<MaterialId>) -> IdKey = IdRef::<MaterialId>::key;
    let _: fn(IdAny) -> Option<IdRef<MaterialId>> = IdRef::<MaterialId>::from_any;
    let _: fn(IdRef<MaterialId>) -> IdAny = IdRef::<MaterialId>::any;
    let _: fn(EmbeddedSlot) -> &'static str = EmbeddedSlot::prop_identifier;
    let _: IdCode = <MaterialId as IdKind>::CODE;
    let _: &str = <MaterialId as IdKind>::COLLECTION;
    let _: &str = <MaterialId as IdKind>::DEFAULT_NAME;
    let _ = |a: IdAny| -> (IdCode, IdKey) { (a.code, a.key) };
    let _ = |h: IdHeader| -> (
        SmolStr,
        bool,
        Option<IdRef<LibraryId>>,
        bool,
        bool,
        u64,
        u32,
        IndexMap<SmolStr, IdProp>,
    ) {
        (
            h.name,
            h.use_fake_user,
            h.library,
            h.is_linked_packed,
            h.is_embedded_data,
            h.generation,
            h.session_uid,
            h.custom,
        )
    };
    let _ = [EmbeddedSlot::NodeTree, EmbeddedSlot::MasterCollection];
    // One marker per IdCode.
    let _: [IdCode; 16] = [
        ObjectId::CODE,
        MeshId::CODE,
        MaterialId::CODE,
        CameraId::CODE,
        LightId::CODE,
        WorldId::CODE,
        ImageId::CODE,
        ArmatureId::CODE,
        ActionId::CODE,
        SceneId::CODE,
        CollectionId::CODE,
        KeyId::CODE,
        NodeTreeId::CODE,
        TextureId::CODE,
        LibraryId::CODE,
        PointCloudId::CODE,
    ];
}

fn id_type<T: IdType>(t: &mut T) {
    let _: &IdHeader = t.header();
    let _: &mut IdHeader = t.header_mut();
    t.for_each_ref(&mut |_: IdAny| {});
    t.remap_ref(t.header().library.unwrap().any(), None);
    let _: &'static [EmbeddedSlot] = t.embedded();
}

// ---------------------------------------------------------------- §9.1–9.4 RNA

fn metadata(s: &'static StructDef, p: &'static PropDef, f: &'static FuncDef) {
    let _: (&str, Option<&'static StructDef>, &str, &str) =
        (s.identifier, s.base, s.name, s.description);
    let _: (&'static [PropDef], &'static [FuncDef], Option<&str>, bool) =
        (s.props, s.funcs, s.name_property, s.is_id);
    let _: (
        &str,
        &str,
        &str,
        &PropKind,
        Subtype,
        Unit,
        ArrayShape,
        PropFlags,
    ) = (
        p.identifier,
        p.name,
        p.description,
        &p.kind,
        p.subtype,
        p.unit,
        p.array,
        p.flags,
    );
    let _: Option<&'static Deprecation> = p.deprecated;
    let _: (
        &str,
        &str,
        &'static [PropDef],
        &'static [PropDef],
        Option<u8>,
        FuncFlags,
    ) = (
        f.identifier,
        f.description,
        f.params,
        f.returns,
        f.keyword_only_from,
        f.flags,
    );
    let _ = [
        PropFlags::READONLY,
        PropFlags::HIDDEN,
        PropFlags::SKIP_SAVE,
        PropFlags::ENUM_FLAG,
        PropFlags::REQUIRED,
        PropFlags::OUTPUT,
        PropFlags::ANIMATABLE,
        PropFlags::PYTHON_ONLY,
        PropFlags::DYNAMIC_ENUM,
        PropFlags::NEVER_NULL,
        PropFlags::ARGUMENT_OPTIONAL,
        PropFlags::DEPRECATED,
        PropFlags::PATH_SUPPORTS_BLEND_RELATIVE,
        PropFlags::CONTEXT_MEMBER,
        PropFlags::SKIP_PRESET,
    ];
    let _ = [
        FuncFlags::NO_SELF,
        FuncFlags::USE_SELF_TYPE,
        FuncFlags::PYTHON_ONLY,
    ];
    match &p.kind {
        PropKind::Bool { default: _ } => {}
        PropKind::Int {
            default: _,
            hard: (_, _),
            soft: (_, _),
        } => {}
        PropKind::Float {
            default: _,
            hard: _,
            soft: _,
            precision: _,
            storage: _,
        } => {}
        PropKind::String {
            default: _,
            max_bytes: _,
        } => {}
        PropKind::Enum {
            items,
            default: _,
            dynamic,
        } => {
            let _: (&'static [EnumItem], Option<&str>) = (items, *dynamic);
        }
        PropKind::Pointer {
            fixed_type: _,
            is_id: _,
        } => {}
        PropKind::Collection {
            item_type: _,
            api: _,
        } => {}
        _ => {}
    }
}

fn values(def: &PropDef) -> Result<(), RnaError> {
    let _: fn(&PropDef, Input, &CoerceCx) -> Result<Coerced, RnaError> = coerce;
    let c = coerce(
        def,
        Input::Math(MathValue::Vector(vec![0.0; 3])),
        &CoerceCx::RnaSet { owner: "Object" },
    )?;
    let _: (Value, bool) = (c.value, c.clamped);
    let _ = CoerceCx::OpArgs {
        owner: "MESH_OT_primitive_cube_add",
    };
    let _ = [
        Input::None,
        Input::Bool(true),
        Input::Int(1),
        Input::Float(1.0),
        Input::Str(String::new()),
        Input::Bare(SmolStr::default()),
        Input::Tuple(vec![]),
        Input::List(vec![]),
        Input::Set(vec![]),
        Input::Dict(IndexMap::new()),
    ];
    let ptr = |p: RnaPtr| Input::Ptr(p);
    let _ = ptr;
    let v = |p: RnaPtr| -> [Value; 17] {
        [
            Value::None,
            Value::Bool(true),
            Value::Int(1),
            Value::Float(1.0),
            Value::Double(1.0),
            Value::Str(SmolStr::default()),
            Value::Enum(SmolStr::default()),
            Value::EnumSet(BTreeSet::new()),
            Value::BoolArray(SmallVec::new()),
            Value::IntArray(SmallVec::new()),
            Value::FloatArray {
                data: SmallVec::<[f32; 16]>::new(),
                shape: ArrayShape::Fixed(3),
                tag: MathTag::Euler(EulerOrder::XYZ),
            },
            Value::Id(None),
            Value::Ptr(Box::new(p.clone())),
            Value::Coll(Box::new(p)),
            Value::Tuple(vec![]),
            Value::Structs(vec![IndexMap::<SmolStr, Value>::new()]),
            Value::from(IdRef::<ObjectId>::from_key(IdKey::next())), // F-VAL-4: cx.result("object", ob)
        ]
    };
    let _ = v;
    let _ = [
        MathTag::Vector,
        MathTag::Quaternion,
        MathTag::AxisAngle,
        MathTag::Matrix,
        MathTag::Color,
        MathTag::None,
    ];
    Ok(())
}

fn paths(names: &dyn IdNames, p: &RnaPtr) {
    let _: (Root, &SmallVec<[Seg; 6]>, &'static StructDef) = (p.root, &p.path, p.srna);
    let _: String = p.repr(names);
    let _: Option<String> = p.path_from_id();
    let _: Option<(SmolStr, Option<SmolStr>)> =
        names.id_name(IdRef::<ObjectId>::from_key(IdKey::next()).any());
    let _ = |a: IdAny| {
        [
            Root::Data,
            Root::Context,
            Root::Id(a),
            Root::Embedded {
                owner: a,
                slot: EmbeddedSlot::NodeTree,
            },
        ]
    };
    let _ = [Seg::Prop("x"), Seg::Index(0), Seg::Key(SmolStr::default())];
    let _ = |r: Resolved| match r {
        Resolved::Struct(_) => {}
        Resolved::Prop {
            ptr: _,
            prop: _,
            index: _,
            component: _,
        } => {}
        Resolved::Coll { ptr: _, prop: _ } => {}
        Resolved::Func { ptr: _, func: _ } => {}
        _ => {}
    };
}

fn traits<'a>(
    r: &'a dyn Rna,
    m: &mut dyn RnaMut,
    c: &'a dyn RnaColl,
    cm: &mut dyn RnaCollMut,
    p: &'static PropDef,
) -> Result<(), RnaError> {
    let _: &'static StructDef = r.srna();
    let f: Field<'a> = r.get(p)?;
    match f {
        Field::Value(_) | Field::Id(_) | Field::NeedsHook | Field::Ptr(_) => {}
        Field::Struct(s) => {
            let _: &dyn Rna = &*s;
        }
        Field::Coll(c) => {
            let _: &dyn RnaColl = &*c;
        }
        Field::Embedded(_slot, _s) => {}
        _ => {}
    }
    let _: SetOutcome = m.set(p, Value::None)?;
    {
        let _g: RnaMutGuard<'_> = m.struct_mut(p)?;
    }
    {
        let _g: RnaCollMutGuard<'_> = m.coll_mut(p)?;
    }
    let _: (&'static StructDef, usize, bool) = (c.item_type(), c.len(), c.is_empty());
    let _: Option<SmolStr> = c.key_of(0);
    let _: Option<usize> = c.find("x");
    let _: Item<'a> = c.item(0)?;
    let _: Seg = c.item_seg(0);
    let _: Bulk = c.foreach_get(p, 0..1_usize)?;
    let _: Range<usize> = 0..1;
    {
        let _g: RnaMutGuard<'_> = cm.item_mut(0)?;
    }
    cm.foreach_set(p, &Bulk::F32(vec![]))?;
    let _ = [SetOutcome::Done, SetOutcome::NeedsHook];
    // F-TR-6: a reborrow of real memory is a guard.
    let _ = |x: &'a mut Object| -> RnaMutGuard<'a> { Box::new(x) };
    let _ = |s: StructRef<'a>, i: Option<IdAny>| [Item::Struct(s), Item::Id(i)];
    Ok(())
}

// ---------------------------------------------------------------- §10.1 operator specs (frap-rna)

fn op_specs(spec: &'static OperatorSpec, a: &OpArgs) {
    let _: (&str, &str, &str, &str, OpOptions) = (
        spec.idname,
        spec.bl_idname,
        spec.label,
        spec.description,
        spec.options,
    );
    let _: (
        &'static [PropDef],
        &'static [&'static str],
        Option<PollMatrix>,
    ) = (spec.props, spec.macro_steps, spec.poll_matrix);
    let _: (
        &IndexMap<&'static str, Value>,
        &BitSet,
        &IndexMap<&'static str, OpArgs>,
    ) = (&a.values, &a.is_set, &a.nested);
    let p: Prop<f32> = a.prop(0, "size");
    let _: Prop<f64> = p.map(|x| x as f64);
    let _ = PollMatrix {
        plain: [PollCell::Y; 5],
        view3d: [PollCell::N; 5],
    };
    let _ = PollCell::Raised;
    fn props<P: OpProps>(a: &OpArgs) -> P {
        let _: (&str, &str, &'static OperatorSpec) = (P::IDNAME, P::BL_IDNAME, P::spec());
        P::from_args(a)
    }
    let _: frap_spec_ops_mesh::mesh::PrimitiveCubeAdd = props(a);
}

// ---------------------------------------------------------------- §13.4 errors

fn errors(e: FrapError, r: RnaError) {
    let _: (PyExc, ErrorCode, String, Option<Span>) = (e.exception, e.code, e.message, e.span);
    let _: (serde_json::Map<String, serde_json::Value>, Option<String>) = (e.detail, e.hint);
    let _: FrapError = r.into();
    let s = Span::default();
    let _: (u32, u32, u32, u32, u32) = (s.start, s.len, s.line, s.col, s.col_chars);
    let _ = [
        PyExc::SyntaxError,
        PyExc::NameError,
        PyExc::TypeError,
        PyExc::ValueError,
        PyExc::KeyError,
        PyExc::IndexError,
        PyExc::AttributeError,
        PyExc::RuntimeError,
        PyExc::NotImplementedError,
    ];
    let rep = Report::new(ReportType::Warning, "x");
    let _: (ReportType, String) = (rep.level, rep.message);
    let _ = [
        ReportType::Info,
        ReportType::Error,
        ReportType::Operator,
        ReportType::Property,
    ];
    let _: &[ErrorCodeDoc] = &ERROR_CODES;
}

// ---------------------------------------------------------------- §11.2 IR

fn ir(stmt: Stmt) {
    let _: (Command, Span, u32, Form) = (stmt.cmd, stmt.span, stmt.line, stmt.form);
    let path = PathExpr {
        root: PathRoot::BpyData,
        segs: vec![
            PathSeg::Attr("objects".into()),
            PathSeg::Key("Cube".into()),
            PathSeg::Index(0),
            PathSeg::Slice(Some(0), None),
            PathSeg::Call(vec![]),
        ],
        span: Span::default(),
    };
    let e = Expr::Lit(Lit::Int(2), Span::default());
    let arg = Arg {
        name: Some("size".into()),
        value: e.clone(),
        span: Span::default(),
    };
    let _ = [
        Command::Invoke {
            idname: "mesh.primitive_cube_add".into(),
            positional: vec![],
            args: vec![arg.clone()],
        },
        Command::OpMeta {
            idname: "mesh.bevel".into(),
            what: OpMetaKind::Poll,
        },
        Command::Get {
            target: path.clone(),
        },
        Command::Set {
            target: path.clone(),
            op: AssignOp::Add,
            value: e.clone(),
        },
        Command::Call {
            target: path.clone(),
            args: vec![arg.clone()],
        },
        Command::Print {
            values: vec![e.clone()],
        },
        Command::Noop {
            reason: NoopReason::Import,
        },
    ];
    let b = Box::new(e.clone());
    let _ = [
        Expr::Bare("WORLD".into(), Span::default()),
        Expr::Path(path.clone()),
        Expr::Tuple(vec![], Span::default()),
        Expr::List(vec![], Span::default()),
        Expr::Set(vec![], Span::default()),
        Expr::Dict(vec![], Span::default()),
        Expr::Unary(UnOp::Neg, b.clone(), Span::default()),
        Expr::Binary(BinOp::MatMul, b.clone(), b.clone(), Span::default()),
        Expr::Builtin {
            name: Builtin::Vector,
            args: vec![],
            span: Span::default(),
        },
        Expr::Method {
            recv: b,
            name: "normalized".into(),
            args: vec![arg],
            span: Span::default(),
        },
    ];
    let _ = [PathRoot::BpyContext];
    let _ = [Form::Bpy, Form::Short];
}

// ---------------------------------------------------------------- §6.1 Mesh

fn mesh(m: &mut Mesh) {
    let _: &AttributeSet = m.attributes();
    let _: &[i32] = m.face_offsets();
    let _: Option<&[DeformVert]> = m.deform_verts();
    let _: &mut AttributeSet = m.attributes_mut();
    m.set_topology(vec![0], vec![], vec![]);
    let _: &mut Vec<DeformVert> = m.deform_verts_mut();
    let _: Arc<Vec<[f32; 3]>> = m.corner_normals();
    let _: (
        &IdHeader,
        &Vec<Option<IdRef<MaterialId>>>,
        &Vec<SmolStr>,
        Option<IdRef<KeyId>>,
    ) = (&m.header, &m.materials, &m.vertex_group_names, m.shape_keys);
    let _: (
        &Option<SmolStr>,
        &Option<SmolStr>,
        &Option<SmolStr>,
        &Option<SmolStr>,
    ) = (
        &m.uv_active,
        &m.uv_active_render,
        &m.color_active,
        &m.color_default,
    );
    let a = Attribute {
        name: "position".into(),
        domain: Domain::Point,
        data: AttrData::Float3(Arc::new(vec![])),
    };
    let _ = [Domain::Edge, Domain::Face, Domain::Corner];
    let _ = [
        AttrData::Float(Arc::new(vec![])),
        AttrData::Int(Arc::new(vec![])),
        AttrData::Int8(Arc::new(vec![])),
        AttrData::Bool(Arc::new(vec![])),
        AttrData::Float2(Arc::new(vec![])),
        AttrData::Float4(Arc::new(vec![])),
        AttrData::FloatColor(Arc::new(vec![])),
        AttrData::ByteColor(Arc::new(vec![])),
        AttrData::Int16x2(Arc::new(vec![])),
        AttrData::Int32x2(Arc::new(vec![])),
        AttrData::Quaternion(Arc::new(vec![])),
        AttrData::Float4x4(Arc::new(vec![])),
        AttrData::String(Arc::new(vec![])),
        a.data,
    ];
    id_type(m);
}

// ---------------------------------------------------------------- §16.1 node trees

fn nodes(t: &mut NodeTree) {
    let _: (
        &IdHeader,
        NodeTreeType,
        &Vec<Node>,
        &Vec<Link>,
        u32,
        &Option<AnimData>,
    ) = (
        &t.header,
        t.ty,
        &t.nodes,
        &t.links,
        t.next_node_uid,
        &t.animation_data,
    );
    let n: &Node = &t.nodes[0];
    let _: (
        u32,
        &SmolStr,
        &String,
        &'static str,
        [f32; 2],
        f32,
        Option<u32>,
        bool,
        bool,
    ) = (
        n.uid,
        &n.name,
        &n.label,
        n.bl_idname,
        n.location,
        n.width,
        n.parent,
        n.mute,
        n.hide,
    );
    let s: &Socket = &n.inputs[0];
    let _: (&SmolStr, &SmolStr, bool, bool) = (&s.name, &s.identifier, s.enabled, s.hide_value);
    let l: &Link = &t.links[0];
    let _: (u32, &SmolStr, u32, &SmolStr) = (l.from_node, &l.from_socket, l.to_node, &l.to_socket);
    id_type(t);
}

// ---------------------------------------------------------------- §5.6 datablock field sets (W0-21)

fn datablocks() {
    use frap_scene::generated::plain::*;
    use frap_scene::id::{
        Camera, Collection, EncodedImage, Image, Light, Modifier, ModifierSettings, Pose, Scene,
        SlotLink, SlotOverride, World,
    };
    let o = Object::new("Cube", None);
    let _: (
        &[f32; 3],
        &SmolStr,
        &[f32; 3],
        &[f32; 4],
        &[f32; 4],
        &[f32; 3],
    ) = (
        &o.location,
        &o.rotation_mode,
        &o.rotation_euler,
        &o.rotation_quaternion,
        &o.rotation_axis_angle,
        &o.scale,
    );
    let _: (&[f32; 3], &[f32; 3], &[f32; 4], &[f32; 3]) = (
        &o.delta_location,
        &o.delta_rotation_euler,
        &o.delta_rotation_quaternion,
        &o.delta_scale,
    );
    let _: (
        &Option<IdRef<ObjectId>>,
        &SmolStr,
        &SmolStr,
        &[i32; 3],
        &glam::Mat4,
    ) = (
        &o.parent,
        &o.parent_type,
        &o.parent_bone,
        &o.parent_vertices,
        &o.matrix_parent_inverse,
    );
    let _: (&Vec<Modifier>, &Vec<SlotOverride>, i32, i32, &Option<Pose>) = (
        &o.modifiers,
        &o.material_slot_overrides,
        o.active_material_index,
        o.vertex_groups_active_index,
        &o.pose,
    );
    let _: (bool, bool, bool, &SmolStr, &[f32; 4], &SmolStr, f32, i32) = (
        o.hide_viewport,
        o.hide_render,
        o.hide_select,
        &o.display_type,
        &o.color,
        &o.empty_display_type,
        o.empty_display_size,
        o.active_shape_key_index,
    );
    let _: &Option<AnimData> = &o.animation_data;
    let so = SlotOverride::default();
    let _: (SlotLink, Option<IdRef<MaterialId>>) = (so.link, so.material);
    let _ = [SlotLink::Data, SlotLink::Object];
    let md = Modifier {
        name: "Bevel".into(),
        show_viewport: true,
        show_render: true,
        show_in_editmode: false,
        show_on_cage: false,
        show_expanded: true,
        is_active: true,
        use_pin_to_last: false,
        persistent_uid: 1,
        settings: ModifierSettings::Untyped {
            r#type: "BEVEL".into(),
        },
    };
    let _ = md;

    let sc = Scene::new("Scene", vec![]);
    let _: (
        &Collection,
        &Option<IdRef<ObjectId>>,
        &Option<IdRef<WorldId>>,
    ) = (&sc.collection, &sc.camera, &sc.world);
    let _: (i32, i32, i32, i32) = (
        sc.frame_start,
        sc.frame_end,
        sc.frame_current,
        sc.frame_step,
    );
    let _: (
        &RenderSettings,
        &ColorManagedViewSettings,
        &ColorManagedDisplaySettings,
        &SceneDisplay,
        &SceneEEVEE,
        &View3DCursor,
        &ToolSettings,
        &UnitSettings,
    ) = (
        &sc.render,
        &sc.view_settings,
        &sc.display_settings,
        &sc.display,
        &sc.eevee,
        &sc.cursor,
        &sc.tool_settings,
        &sc.unit_settings,
    );
    let _: (&SmolStr, i32, i32, i32, &String, bool, &ImageFormatSettings) = (
        &sc.render.engine,
        sc.render.resolution_x,
        sc.render.resolution_y,
        sc.render.resolution_percentage,
        &sc.render.filepath,
        sc.render.film_transparent,
        &sc.render.image_settings,
    );
    let _: &View3DShading = &sc.display.shading;
    assert!(sc.collection.header.is_embedded_data);
    assert_eq!(sc.collection.header.name, "Scene Collection");
    let c = Collection::new("C");
    let _: (bool, bool, bool, &SmolStr) =
        (c.hide_select, c.hide_viewport, c.hide_render, &c.color_tag);

    let m = Material::new("M");
    let _: (
        &NodeTree,
        &SmolStr,
        &SmolStr,
        bool,
        &[f32; 4],
        f32,
        f32,
        f32,
        i32,
        bool,
    ) = (
        &m.node_tree,
        &m.surface_render_method,
        &m.blend_method,
        m.use_backface_culling,
        &m.diffuse_color,
        m.metallic,
        m.roughness,
        m.alpha_threshold,
        m.pass_index,
        m.use_nodes,
    );
    assert!(m.node_tree.header.is_embedded_data);
    assert_eq!(
        m.node_tree.header.name,
        frap_scene::id::SHADER_NODETREE_NAME
    );

    let cam = Camera::new("Camera");
    let f: &CameraFields = &cam.fields;
    let _: (&SmolStr, f32, f32, f32, &SmolStr, f32, f32, f32, f32, f32) = (
        &f.r#type,
        f.lens,
        f.sensor_width,
        f.sensor_height,
        &f.sensor_fit,
        f.shift_x,
        f.shift_y,
        f.clip_start,
        f.clip_end,
        f.ortho_scale,
    );
    let _: &CameraDOFSettings = &f.dof;
    let l = Light::new("Light", "SUN");
    let lf: &LightFields = &l.fields;
    let _: (
        &SmolStr,
        &[f32; 3],
        f32,
        f32,
        f32,
        f32,
        f32,
        &SmolStr,
        f32,
        f32,
    ) = (
        &l.r#type,
        &lf.color,
        lf.energy,
        lf.shadow_soft_size,
        lf.spot_size,
        lf.spot_blend,
        lf.angle,
        &lf.shape,
        lf.size,
        lf.size_y,
    );
    let w = World::new("World");
    let _: (&NodeTree, &[f32; 3], bool) = (&w.node_tree, &w.color, w.use_nodes);
    let im = Image::new("I", 4, 4);
    let _: (
        &SmolStr,
        &String,
        &Option<Arc<EncodedImage>>,
        &SmolStr,
        [i32; 2],
    ) = (
        &im.source,
        &im.filepath,
        &im.packed_file,
        &im.alpha_mode,
        im.size,
    );
    let _: (&SmolStr, i32, i32, &[f32; 4], bool, &SmolStr) = (
        &im.generated_type,
        im.generated_width,
        im.generated_height,
        &im.generated_color,
        im.use_generated_float,
        &im.colorspace_settings.name,
    );
    let _ = Pose::default();
}

// ---------------------------------------------------------------- §5.2, §8, §9.5 scene

fn main_and_maps(main: &mut Main, log: &mut ChangeLog) {
    let _: (&IdMap<Object>, &IdMap<Mesh>, &IdMap<Material>) =
        (&main.objects, &main.meshes, &main.materials);
    let _ = (
        &main.cameras,
        &main.lights,
        &main.worlds,
        &main.images,
        &main.armatures,
        &main.scenes,
        &main.collections,
        &main.shape_keys,
        &main.textures,
        &main.libraries,
        &main.pointclouds,
    );
    let _: (&IdMap<Action>, &IdMap<NodeTree>, &Option<String>, bool) = (
        &main.actions,
        &main.node_groups,
        &main.filepath,
        main.is_dirty,
    );
    let r: IdRef<MaterialId> = main.materials.find("Material").unwrap();
    let m = &mut main.materials;
    let _: Option<&Material> = m.get(r);
    let _: Option<Arc<Material>> = m.get_arc(r);
    let _: Option<&mut Material> = m.get_mut(r, log);
    let _: Option<IdRef<MaterialId>> = m.find_in("Material", None);
    let _: Option<usize> = m.index_of(r);
    let _: Option<IdRef<MaterialId>> = m.at(-1);
    let _ = m
        .iter()
        .map(|(r, t): (IdRef<MaterialId>, &Material)| (r, t.header.name.clone()));
    let _: frap_scene::RenameResult = m.rename(r, "M", frap_scene::RenameMode::Never, log);
    let _: Option<Arc<Material>> = m.remove(r, log);
    let a = r.any();
    let _: Option<&dyn Rna> = main.id(a);
    let _: Option<&mut dyn RnaMut> = main.id_mut(a, log);
    let _: Option<&IdHeader> = main.header(a);
    let _: Option<(Vec<IdAny>, &'static StructDef)> = main.collection("objects");
    let _: &dyn IdNames = main;
    let _: (
        &Vec<IdAny>,
        &Vec<(IdAny, SmolStr)>,
        &indexmap::IndexSet<IdAny>,
        &Vec<(IdAny, SmolStr)>,
        bool,
    ) = (
        &log.added,
        &log.removed,
        &log.modified,
        &log.renamed,
        log.context_changed,
    );
}

fn context(c: Context<'_>, w: &WindowState, e: &EditState) {
    let _: (&Main, &WindowState, &frap_scene::Preferences) = (c.main, c.win, c.prefs);
    let _: &ViewLayer = c.view_layer();
    let _: Option<IdRef<ObjectId>> = c.active_object();
    let _: Option<IdRef<ObjectId>> = c.object();
    let _: Vec<IdRef<ObjectId>> = c.selected_objects();
    let _f = || -> Vec<IdRef<ObjectId>> { c.selected_editable_objects() };
    let _f = || -> Vec<IdRef<ObjectId>> { c.visible_objects() };
    let _: ContextMode = c.mode();
    let _f = || -> Option<IdRef<ObjectId>> { c.edit_object() };
    let _f = || -> Vec<IdRef<ObjectId>> { c.objects_in_mode() };
    let _f = || -> Vec<IdRef<ObjectId>> { c.objects_in_mode_unique_data() };
    let _: Option<IdRef<CollectionId>> = c.collection();
    let _ = (c.tool_settings(), c.cursor());
    let _f = || -> Option<frap_scene::BoneRef> { c.active_bone() };
    let _f = || -> Option<frap_scene::BoneRef> { c.active_pose_bone() };
    let _f = || -> Vec<frap_scene::BoneRef> { c.selected_pose_bones() };
    let _f = || -> Option<IdRef<ObjectId>> { c.pose_object() };
    let _f = |e: &frap_scene::EditState| -> Option<frap_scene::BoneRef> { c.active_edit_bone(e) };
    let _f = |a: &frap_scene::id::Armature, b: &frap_scene::id::Bone| -> bool {
        frap_scene::bone_in_visible_collection(a, b)
    };
    let _: (IdRef<SceneId>, &SmolStr, &frap_scene::VirtualView3D) =
        (w.scene, &w.view_layer, &w.view3d);
    let vl = c.view_layer();
    let _: (
        &SmolStr,
        &Vec<frap_scene::Base>,
        Option<IdRef<ObjectId>>,
        &frap_scene::LayerCollection,
        &frap_scene::LayerCollectionPath,
    ) = (
        &vl.name,
        &vl.bases,
        vl.active,
        &vl.layer_collection,
        &vl.active_layer_collection,
    );
    let b = &vl.bases[0];
    let _: (IdRef<ObjectId>, frap_scene::BaseFlag) = (b.object, b.flag);
    let _: (
        &BTreeMap<IdKey, EditMesh>,
        &BTreeMap<IdKey, Arc<frap_scene::EditArmature>>,
    ) = (&e.meshes, &e.armatures);
    let em = &e.meshes[&IdKey::next()];
    let _: (&Arc<BMesh>, u64, &frap_scene::ViewCache) = (&em.bm, em.bm_gen, &em.view);
}

fn host_read<H: HostRead + ?Sized>(h: &H, me: IdRef<MeshId>) {
    let _: (&Main, &WindowState, &EditState, Context<'_>) = (h.main(), h.win(), h.edit(), h.ctx());
    let _: Arc<Mesh> = h.mesh_view(me);
}

fn host<H: Host + ?Sized>(h: &mut H) -> Result<(), RnaError> {
    {
        let _: (&mut Main, &mut ChangeLog) = h.main_mut();
    }
    {
        let _: &mut EditState = h.edit_mut();
    }
    {
        let (_c, _e): (Context<'_>, &mut EditState) = h.split_edit(); // F-OP-5
    }
    {
        let _: &mut WindowState = h.win_mut();
    }
    {
        let (_r, _e): (&dyn HostRead, &mut dyn Evaluator) = h.eval();
    }
    let s: Snapshot = h.snapshot_at(None)?; // F-OP-7
    let _: (Box<dyn HostRead + Send>, Box<dyn Evaluator + Send>) = (s.host, s.eval);
    h.mark_time_dirty();
    h.apply_pending_animation()?;
    h.report(Report::new(ReportType::Info, "x"));
    h.set_path(Root::Data, "location", 0, Value::None)?;
    Ok(())
}

fn evaluator(
    e: &mut dyn Evaluator,
    h: &dyn HostRead,
    ob: IdRef<ObjectId>,
) -> Result<(), EvalError> {
    let o: Arc<EvaluatedObject> = e.object(h, ob, EvalMode::Viewport)?;
    let _: (glam::Mat4, &Option<Arc<Mesh>>, Option<[[f32; 3]; 2]>) =
        (o.matrix_world, &o.mesh, o.bounds);
    let _: (
        &Option<Arc<frap_scene::EvaluatedPose>>,
        &Vec<(SmolStr, frap_scene::ModifierStatus)>,
        &Vec<Report>,
    ) = (&o.pose, &o.modifier_status, &o.warnings);
    e.depsgraph_update(h);
    let _ = EvalMode::Render;
    // frap-eval implements it.
    let _: Box<dyn Evaluator + Send> = Box::new(frap_eval::EvalCache::new());
    Ok(())
}

fn hooks() {
    let _: &[RnaFuncReg] = &RNA_FUNCS;
    let _: &[RnaHookReg] = &RNA_HOOKS;
    fn f(_: &mut dyn Host, _: &RnaPtr, _: &mut FuncArgs) -> Result<Value, RnaError> {
        Ok(Value::None)
    }
    fn s(_: &mut dyn Host, _: &RnaPtr, _: &'static PropDef, _: Value) -> Result<(), RnaError> {
        Ok(())
    }
    // F-HOOK-1: reads take &dyn HostRead and may return a proxy borrowing the scene.
    fn g<'h>(_: &'h dyn HostRead, _: &RnaPtr, _: &'static PropDef) -> Result<Field<'h>, RnaError> {
        Ok(Field::NeedsHook)
    }
    let _: RnaFuncImpl = f;
    let _: RnaSetHook = s;
    let _: RnaGetHook = g;
    let _ = RnaFuncReg {
        srna: "Object",
        func: "select_set",
        imp: f,
    };
    let _ = RnaHookReg {
        srna: "Object",
        prop: "rotation_mode",
        get: Some(g),
        set: Some(s),
    };
    let _: &[frap_rna::testing::HookDecl] = &frap_rna::testing::HOOK_DECLS;
}

// ---------------------------------------------------------------- §10.1 framework (frap-op)

fn op_defs(def: &'static OperatorDef) {
    let _: (
        &'static OperatorSpec,
        Poll,
        fn(&mut OpCtx, &OpArgs) -> OpResult,
    ) = (def.spec, def.poll, def.exec);
    let _: (
        OpKind,
        &'static [(&'static str, DynEnumFn)],
        Tier,
        Support,
        &'static [&'static str],
    ) = (def.kind, def.dyn_enums, def.tier, def.support, def.notes);
    let _: &[OperatorDef] = &OPERATORS;
    let p = def.poll;
    let _: (&'static [PollReq], fn(&Context) -> Result<(), PollFail>) = (p.requires, p.check);
    let pf = PollFail {
        message: None,
        unmet: SmallVec::new(),
        hint: None,
    };
    let _: (Option<String>, SmallVec<[PollReq; 2]>, Option<String>) =
        (pf.message, pf.unmet, pf.hint);
    let _ = [OpStatus::Finished, OpStatus::Cancelled];
    let _ = |e: RnaError| {
        [
            OpError::Runtime(String::new()),
            OpError::Rna(e),
            OpError::NotImplemented(String::new()),
            OpError::Io(String::new()),
            OpError::Gpu(String::new()),
            OpError::Limit(String::new()),
        ]
    };
}

fn services(s: &mut dyn Services) -> Result<(), OpError> {
    {
        let _: &mut dyn RenderService = s.render()?;
    }
    {
        let _: &mut dyn ImageCache = s.images();
    }
    let _: &PathResolver = s.paths();
    Ok(())
}

fn opctx<'h>(
    h: &'h mut dyn Host,
    s: &'h mut dyn Services,
    spec: &'static OperatorSpec,
) -> OpResult {
    let mut cx: OpCtx<'h> = OpCtx::new(h, s, spec); // F-OP-8
    let ob = IdRef::<ObjectId>::from_key(IdKey::next());
    let me = IdRef::<MeshId>::from_key(IdKey::next());
    {
        let _: Context<'_> = cx.ctx();
    }
    {
        let _: &Main = cx.main();
    }
    {
        let _: (&mut Main, &mut ChangeLog) = cx.main_mut(); // F-OP-9
    }
    {
        let _: Result<&mut Object, OpError> = cx.object_mut(ob);
    }
    {
        let _: Result<&mut Mesh, OpError> = cx.mesh_mut(me);
    }
    {
        let _: Result<&mut BMesh, OpError> = cx.bmesh_mut(me);
    }
    {
        let (_c, _e): (Context<'_>, &mut EditState) = cx.split_edit();
    }
    {
        let (_h, _e): (&dyn HostRead, &mut dyn Evaluator) = cx.split_eval(); // F-OP-6
    }
    {
        let _: &mut dyn Services = cx.services();
    }
    let _: Result<Snapshot, OpError> = cx.snapshot_at(Some(1));
    let at = Placement {
        location: [0.0; 3],
        rotation: [0.0; 3],
        scale: [1.0; 3],
    };
    let o: IdRef<ObjectId> = cx.add_object("Cube", None, at);
    let _: Result<Arc<EvaluatedObject>, OpError> = cx.evaluated(o);
    cx.report(ReportType::Warning, "x");
    cx.result("object", o); // §10.4: IdRef<K>: Into<Value> (F-VAL-4)
    let r: OpResult = cx.run_op("mesh.primitive_cube_add", OpArgs::default());
    let out: OpOutput = cx.finish();
    let _: (Vec<Report>, IndexMap<SmolStr, Value>) = (out.reports, out.result);
    r
}

fn extension_points() {
    let _: &[ModeHandler] = &MODE_HANDLERS;
    let _: &[TransformModeHandler] = &TRANSFORM_MODE_HANDLERS;
    fn mode(_: &mut OpCtx, _: &[IdRef<ObjectId>]) -> OpResult {
        Ok(OpStatus::Finished)
    }
    fn tr(_: &mut OpCtx, _: TransformOp, _: &OpArgs) -> OpResult {
        Ok(OpStatus::Finished)
    }
    let _ = ModeHandler {
        mode: frap_scene::ObjectMode::Edit,
        object_type: "MESH",
        enter: mode,
        exit: mode,
    };
    let _ = TransformModeHandler {
        mode: ContextMode::Object,
        apply: tr,
    };
    // §17.2 glTF primitive slices.
    use frap_io_gltf::{
        AttrColumn, AttrValues, MeshExportCx, PRIMITIVE_ATTRIBUTE_WRITERS, PRIMITIVE_EXTRACTORS,
        PRIMITIVE_POST_PROCESS, PrimitiveAttributeWriter, PrimitiveDraft, PrimitiveExtractor,
        PrimitivePostProcess, SplitKey,
    };
    let _: &[PrimitiveExtractor] = &PRIMITIVE_EXTRACTORS;
    let _: &[PrimitiveAttributeWriter] = &PRIMITIVE_ATTRIBUTE_WRITERS;
    let _: &[PrimitivePostProcess] = &PRIMITIVE_POST_PROCESS;
    fn ex(cx: &MeshExportCx, _: &SplitKey, _: &mut Vec<PrimitiveDraft>) {
        // Per-object data (vertex groups, armature, object-linked slots) for M6 (W0-28).
        let _: Option<frap_rna::IdRef<frap_rna::ObjectId>> = cx.object;
        let _ = (cx.main, cx.mesh, cx.settings);
    }
    fn _mesh_export_cx<'a>(
        main: &'a frap_scene::Main,
        mesh: &'a frap_mesh::Mesh,
        object: Option<frap_rna::IdRef<frap_rna::ObjectId>>,
        settings: &'a frap_io_gltf::ExportSettings,
    ) -> MeshExportCx<'a> {
        MeshExportCx::new(main, mesh, object, settings)
    }
    fn key(_: &MeshExportCx, k: &mut SplitKey) -> Result<(), frap_io_gltf::ExportError> {
        k.push("normal", 3, vec![0; 3 * k.corners]);
        Ok(())
    }
    fn wr(
        _: &MeshExportCx,
        _: &SplitKey,
        p: &mut PrimitiveDraft,
    ) -> Result<(), frap_io_gltf::ExportError> {
        // Later milestones' attributes go into the ordered map, not new fields (M1 W0 fix).
        p.attributes.insert(
            "COLOR_0".into(),
            AttrColumn {
                values: AttrValues::U16x4(vec![[65535; 4]; p.positions.len()]),
                normalized: true,
            },
        );
        Ok(())
    }
    let mut k = SplitKey::new(2);
    k.push("normal", 3, vec![1, 2, 3, 4, 5, 6]);
    k.push("texcoord", 2, vec![7, 8, 9, 10]);
    assert_eq!(k.stride(), 5);
    assert_eq!(k.corner(1).collect::<Vec<_>>(), [4, 5, 6, 9, 10]);
    assert_eq!(k.column("texcoord").map(|c| c.stride), Some(2));
    fn pp(
        _: &MeshExportCx,
        _: &mut PrimitiveDraft,
        _: &mut frap_gltf::BufferBuilder,
    ) -> Result<(), frap_io_gltf::ExportError> {
        Ok(())
    }
    let _ = PrimitiveExtractor {
        name: "triangles",
        order: 0,
        run: ex,
    };
    let _ = PrimitiveAttributeWriter {
        name: "normal",
        order: 0,
        key: Some(key),
        run: wr,
    };
    let _ = PrimitivePostProcess {
        name: "quantize",
        order: 0,
        run: pp,
    };
}

// ---------------------------------------------------------------- §16.2 LoweredMaterial

fn lowered(lm: &LoweredMaterial, tree: &NodeTree, mat: &mut Material, images: &dyn ImageLookup) {
    let _: (
        &Channel<[f32; 4]>,
        &Channel<f32>,
        &Channel<f32>,
        &Option<NormalMap>,
        &Option<TexRef>,
    ) = (
        &lm.base_color,
        &lm.metallic,
        &lm.roughness,
        &lm.normal,
        &lm.occlusion,
    );
    let _: (&Channel<[f32; 3]>, f32, AlphaMode, bool, bool) = (
        &lm.emissive,
        lm.emissive_strength,
        lm.alpha_mode,
        lm.double_sided,
        lm.unlit,
    );
    let _: (&MaterialExts, &Option<VertexColorUse>, &Vec<Report>) =
        (&lm.ext, &lm.vertex_color, &lm.diagnostics);
    let c = &lm.base_color;
    let _: ([f32; 4], &Option<TexRef>) = (c.factor, &c.texture);
    let _: fn(&NodeTree, &Material, &dyn ImageLookup) -> LoweredMaterial = frap_material::lower;
    let _: fn(&LoweredMaterial, &mut Material, &mut NameAlloc) = frap_material::raise;
    // §16.2 pattern-recogniser slice (extension point, W0-28).
    {
        use frap_material::{LowerCx, MATERIAL_PATTERNS, MaterialPattern, RaiseCx};
        let _: &[MaterialPattern] = &MATERIAL_PATTERNS;
        fn lo(cx: &LowerCx, out: &mut LoweredMaterial) {
            let _ = (cx.tree, cx.mat, cx.images, &mut out.diagnostics);
        }
        fn ra(_: &LoweredMaterial, cx: &mut RaiseCx) {
            let _ = (&mut *cx.into, &mut *cx.names);
        }
        let _ = MaterialPattern {
            name: "base_color",
            order: 0,
            lower: lo,
            raise: Some(ra),
        };
        fn _lower_cx<'a>(t: &'a NodeTree, m: &'a Material, i: &'a dyn ImageLookup) -> LowerCx<'a> {
            LowerCx::new(t, m, i)
        }
        fn _raise_cx<'a>(m: &'a mut Material, n: &'a mut NameAlloc) -> RaiseCx<'a> {
            RaiseCx::new(m, n)
        }
    }
    let _ = (
        tree,
        mat,
        images,
        AlphaMode::Mask { cutoff: 0.5 },
        AlphaMode::Blend,
        AlphaMode::Opaque,
    );
}

// ---------------------------------------------------------------- §19.2 render

fn render(rs: &RenderScene, svc: &mut dyn RenderService) -> Result<(), RenderError> {
    let _: [u32; 2] = rs.resolution;
    let _ = (&rs.camera, &rs.objects, &rs.lights, &rs.world, &rs.settings);
    let _: &IndexMap<frap_render_scene::MeshKey, Arc<RMesh>> = &rs.meshes;
    let _: &IndexMap<frap_render_scene::MatKey, Arc<LoweredMaterial>> = &rs.materials;
    let _: &IndexMap<frap_render_scene::ImageKey, Arc<frap_imbuf::DecodedImage>> = &rs.images;
    let film: Film = svc.render(rs)?;
    let _: (u32, u32, Vec<[f32; 4]>) = (film.width, film.height, film.rgba);
    let _: Vec<Film> = svc.render_tiles(std::slice::from_ref(rs))?;
    let _: AdapterSummary = svc.adapter_info();
    let m = RMesh::default();
    let _: (
        &Vec<[f32; 3]>,
        &Vec<[f32; 3]>,
        &Vec<Vec<[f32; 2]>>,
        &Vec<Vec<[f32; 4]>>,
    ) = (&m.positions, &m.normals, &m.uvs, &m.colors);
    let _: (
        &Option<Vec<[f32; 4]>>,
        &Vec<u32>,
        &Vec<(u32, Range<u32>)>,
        &Vec<[u32; 2]>,
    ) = (&m.tangents, &m.indices, &m.submeshes, &m.edges);
    // frap-render (L9).
    let _: fn(&GpuOptions) -> Result<Box<dyn RenderService>, RenderError> =
        frap_render::new_service;
    let _: fn(&GpuOptions) -> Result<frap_render::Gpu, RenderError> = frap_render::Gpu::new;
    fn engine<E: frap_render::Engine>(
        e: &mut E,
        gpu: &frap_render::Gpu,
        cache: &mut frap_render::GpuCache,
        s: &RenderScene,
    ) -> Result<Film, RenderError> {
        let _ = (&gpu.device, &gpu.queue, &gpu.info); // wgpu::Device, wgpu::Queue, wgpu::AdapterInfo
        e.render(gpu, cache, s)
    }
    let _ = engine::<NoEngine>;
    // frap-render-scene builder (§19.1).
    let _: fn(
        &dyn HostRead,
        &mut dyn Evaluator,
        &RenderRequest,
    ) -> Result<RenderScene, RenderError> = frap_render_scene::build;
    Ok(())
}

struct NoEngine;
impl frap_render::Engine for NoEngine {
    fn render(
        &mut self,
        _: &frap_render::Gpu,
        _: &mut frap_render::GpuCache,
        _: &RenderScene,
    ) -> Result<Film, RenderError> {
        Ok(Film::default())
    }
}

#[test]
fn contract_signatures_compile_and_cheap_ones_run() {
    // Calling the non-todo!() pieces keeps this honest beyond type checking.
    identity();
    let e = Report::new(ReportType::Info, "m");
    assert_eq!(e.level.as_str(), "INFO");
    assert_eq!(IdCode::NT.collection(), "node_groups");
    assert_eq!(EmbeddedSlot::NodeTree.prop_identifier(), "node_tree");
    let ob = IdRef::<ObjectId>::from_key(IdKey::next());
    assert_eq!(IdRef::<ObjectId>::from_any(ob.any()), Some(ob));
    assert_eq!(IdRef::<MeshId>::from_any(ob.any()), None);
    assert_eq!(Value::from(ob), Value::Id(Some(ob.any())));
    let mut bits = BitSet::new();
    bits.insert(70);
    assert!(bits.contains(70) && !bits.contains(6));
    assert_eq!(bits.iter().collect::<Vec<_>>(), vec![70]);
    let _ = NoEngine;
    datablocks();
}

// ---------------------------------------------------------------- M2 W0 contract delta

/// `frap-bmesh` public API (§6.2, frozen at M2 W0; bodies are WP 2.1's / the W2 WPs').
fn bmesh_api(bm: &mut BMesh, me: &Mesh) {
    use frap_bmesh::{
        BMEdge, BMElem, BMFace, BMLoop, BMVert, BmError, CdLayer, CreateFlag, CustomData, DiskLink,
        EdgeId, ElemFlag, ElemTypes, FaceId, FromMeshParams, LoopId, Pool, SelectMode, ShapeLayer,
        ToMeshParams, VertId,
    };
    let _: (&Pool<BMVert>, &Pool<BMEdge>, &Pool<BMLoop>, &Pool<BMFace>) =
        (&bm.verts, &bm.edges, &bm.loops, &bm.faces);
    let _: (&CustomData, &CustomData, &CustomData, &CustomData) =
        (&bm.vdata, &bm.edata, &bm.ldata, &bm.pdata);
    let _: (SelectMode, &Vec<BMElem>, Option<FaceId>, &Vec<ShapeLayer>) = (
        bm.select_mode,
        &bm.select_history,
        bm.act_face,
        &bm.shape_layers,
    );
    let _: fn(&mut Pool<BMVert>, BMVert) -> u32 = Pool::alloc;
    let _: fn(&mut Pool<BMVert>, u32) = Pool::free;
    let _: fn(&Pool<BMVert>) -> usize = Pool::live;
    let v = VertId(0);
    let e = EdgeId(0);
    let f = FaceId(0);
    let l = LoopId(0);
    let _: fn(&mut BMesh, [f32; 3], Option<VertId>, CreateFlag) -> VertId = BMesh::vert_create;
    let _: fn(&mut BMesh, VertId, VertId, Option<EdgeId>, CreateFlag) -> EdgeId =
        BMesh::edge_create;
    let _: fn(&mut BMesh, &[VertId], &[EdgeId], Option<FaceId>, CreateFlag) -> Option<FaceId> =
        BMesh::face_create;
    let _: fn(&mut BMesh, &[VertId], Option<FaceId>, CreateFlag, bool) -> Option<FaceId> =
        BMesh::face_create_verts;
    let _: fn(&mut BMesh, VertId, EdgeId) -> (VertId, EdgeId) = BMesh::split_edge_make_vert;
    let _: fn(
        &mut BMesh,
        FaceId,
        LoopId,
        LoopId,
        Option<EdgeId>,
        bool,
    ) -> Option<(FaceId, LoopId)> = BMesh::split_face_make_edge;
    let _: fn(&mut BMesh, FaceId, FaceId, EdgeId) -> Option<FaceId> = BMesh::join_face_kill_edge;
    let _: fn(&mut BMesh, ElemTypes) = BMesh::elem_index_ensure;
    let _: fn(&BMesh, VertId) -> Vec<EdgeId> = BMesh::vert_edges;
    let _: fn(&BMesh, FaceId) -> Vec<LoopId> = BMesh::face_loops;
    let _: fn(&BMesh, VertId, VertId) -> Option<EdgeId> = BMesh::edge_exists;
    let _: fn(&mut BMesh, BMElem, bool) = BMesh::select_set;
    let _: fn(&mut BMesh) = BMesh::select_flush_mode;
    let _: fn(&mut BMesh, SelectMode, bool) = BMesh::select_mode_convert;
    let _: fn(&BMesh, EdgeId) -> Vec<EdgeId> = BMesh::walk_edge_loop;
    let _: fn(&Mesh, &FromMeshParams) -> BMesh = BMesh::from_mesh;
    let _: fn(&BMesh, &Mesh, &ToMeshParams) -> Mesh = BMesh::to_mesh;
    let _: fn(&BMesh) -> Vec<String> = BMesh::validate;
    let _: fn(&mut BMesh, LoopId, FaceId, bool) = BMesh::loop_interp_from_face;
    let _: (&Vec<CdLayer>, ElemFlag, DiskLink) = (
        &bm.vdata.layers,
        ElemFlag::SELECT,
        DiskLink { next: e, prev: e },
    );
    let _ = (v, f, l, me);
    use frap_bmesh::ops::extrude_face_region::{ExtrudeFaceRegionIn, ExtrudeFaceRegionOut};
    let _: fn(&mut BMesh, &ExtrudeFaceRegionIn) -> Result<ExtrudeFaceRegionOut, BmError> =
        frap_bmesh::ops::extrude_face_region::extrude_face_region;
}

/// Other M2 W0 extension points and helpers.
fn m2_extension_points() {
    let _: fn() -> Option<&'static frap_op::DescribeProvider> = frap_op::describe_provider;
    let _: &[frap_op::DescribeProvider] = &frap_op::DESCRIBE_PROVIDERS;
    let _: fn(&mut OpCtx, IdRef<ObjectId>, Mesh, Placement) -> Result<IdRef<ObjectId>, OpError> =
        frap_op::helpers::edit_merge::merge_into_edit_mesh;
    let _: fn(&mut OpCtx) -> Result<(), OpError> = frap_op::helpers::edit_mode::reenter_after_load;
    let _: fn(
        &mut dyn RenderService,
        &[Film],
        &frap_render_scene::SheetLayout,
    ) -> Result<Film, RenderError> = |s, f, l| s.composite_sheet(f, l);
    let w = frap_render_scene::RWorkbenchShading::default();
    let _: (&str, bool, f32, [f32; 3]) = (
        &w.shading_type,
        w.show_xray,
        w.xray_alpha,
        w.light_direction,
    );
    let _: fn(Option<u64>, Option<u64>) -> Result<(), frap_math::budget::ElementsExceeded> =
        frap_math::budget::reserve_corners;
    let _: fn() = frap_math::budget::check;
}

#[test]
fn m2_contract_delta_cheap_parts_run() {
    let bm = BMesh::new();
    assert_eq!(bm.select_mode, frap_bmesh::SelectMode::VERT);
    assert!(bm.select_history_active().is_none());
    let ws = frap_render_scene::RSettings::default().workbench;
    assert_eq!(ws.render_aa, "8");
    assert!(frap_op::describe_provider().is_none() || !frap_op::DESCRIBE_PROVIDERS.is_empty());
    let i = frap_bmesh::ops::bevel::BevelIn::default();
    assert_eq!((i.offset_type, i.vmesh_method), ("OFFSET", "ADJ"));
    let _ = (
        bmesh_api as fn(&mut BMesh, &Mesh),
        m2_extension_points as fn(),
    );
}

// ---------------------------------------------------------------- M3+M4 W0 contract delta

/// §7.2 modifier plugins (`frap-modifiers`, frozen at M4 W0) and the backends they reach.
fn modifier_contract(cx: &mut dyn frap_modifiers::ModEvalCx, md: &frap_modifiers::Modifier) {
    use frap_modifiers::{ModError, ModKind, ModTypeFlags, ModifierType, ModifierTypeInfo};
    let _: &[ModifierTypeInfo] = &frap_modifiers::MODIFIER_TYPES;
    let _ = |i: &ModifierTypeInfo| -> (
        ModifierType,
        ModKind,
        ModTypeFlags,
        fn(&frap_modifiers::ModifierSettings) -> SmallVec<[IdAny; 2]>,
        fn(&frap_modifiers::ModifierSettings) -> Option<&'static str>,
        fn(
            &frap_modifiers::Modifier,
            Mesh,
            &mut dyn frap_modifiers::ModEvalCx,
        ) -> Result<Mesh, ModError>,
    ) { (i.ty, i.kind, i.flags, i.depends_on, i.is_disabled, i.eval) };
    let _: &Main = cx.main();
    let _: &Object = cx.object();
    let _: IdRef<ObjectId> = cx.object_ref();
    let _: Result<Arc<Mesh>, ModError> = cx.evaluated_mesh(cx.object_ref());
    let _: glam::Mat4 = cx.object_matrix_world(cx.object_ref());
    let _: Result<Arc<frap_scene::EvaluatedPose>, ModError> = cx.pose(cx.object_ref());
    let _: EvalMode = cx.mode();
    let _: f32 = cx.frame();
    cx.report(Report::new(ReportType::Warning, "x"));
    let _: &dyn frap_subdiv::Subdivider = cx.subdivider();
    let _: &dyn frap_boolean::BooleanSolver = cx.boolean_solver();
    let _: fn(frap_modifiers::ModifierType) -> Option<&'static ModifierTypeInfo> =
        frap_modifiers::registry::find;
    let _: fn(
        Arc<Mesh>,
        bool,
        &mut dyn frap_modifiers::ModEvalCx,
    ) -> frap_modifiers::stack::StackResult = frap_modifiers::stack::evaluate;
    let _ = md;
}

/// `frap-subdiv`, `frap-boolean`, `frap-uv` (M3+M4 W0 contracts).
fn backends() {
    let _: fn() -> &'static dyn frap_subdiv::Subdivider = frap_subdiv::default_subdivider;
    let _ = |s: &dyn frap_subdiv::Subdivider,
             t: &frap_subdiv::SubdivTopology<'_>,
             p: &frap_subdiv::SubdivSettings|
     -> Result<frap_subdiv::SubdivResult, frap_subdiv::SubdivError> {
        s.subdivide(t, p)
    };
    let _: fn() -> &'static dyn frap_boolean::BooleanSolver = frap_boolean::default_solver;
    let _ = |s: &dyn frap_boolean::BooleanSolver,
             m: &[frap_boolean::BoolMesh<'_>],
             o: &frap_boolean::BoolOptions|
     -> Result<frap_boolean::BoolResult, frap_boolean::BoolError> { s.run(m, o) };
    let _: fn() -> frap_uv::ParamMesh = frap_uv::ParamMesh::new;
    let _: fn(&mut frap_uv::ParamMesh, &frap_uv::ParamFace<'_>) = frap_uv::ParamMesh::face_add;
    let _: fn(&mut frap_uv::ParamMesh, [frap_uv::ParamKey; 2]) = frap_uv::ParamMesh::edge_set_seam;
    let _: fn(&mut frap_uv::ParamMesh, frap_uv::ConstructOpts) -> u32 =
        frap_uv::ParamMesh::construct_end;
    let _: fn(&mut frap_uv::ParamMesh, bool, bool, bool) = frap_uv::ParamMesh::lscm_begin;
    let _: fn(&mut frap_uv::ParamMesh) -> frap_uv::SolveCount = frap_uv::ParamMesh::lscm_solve;
    let _: fn(&mut frap_uv::ParamMesh, &frap_uv::PackParams) = frap_uv::ParamMesh::pack;
    let _: fn(&mut frap_uv::ParamMesh, bool, bool, bool) = frap_uv::ParamMesh::average;
    let _: fn(&frap_uv::ParamMesh, &mut [[f32; 2]]) = frap_uv::ParamMesh::flush;
    let _: fn(&mut [frap_uv::PackIsland], &frap_uv::PackParams) -> f32 =
        frap_uv::pack::pack_islands;
    let _: fn(&[[f32; 3]], &[f32], f32, f32) -> Vec<frap_uv::smart::SmartGroup> =
        frap_uv::smart::project_groups;
    let _: fn([f32; 3]) -> [f32; 2] = frap_uv::project::map_to_tube;
    let _: fn([f32; 3]) -> [f32; 2] = frap_uv::project::map_to_sphere;
    let _: fn(&[[f32; 3]], &[[f32; 3]], &[[f32; 2]], &[&[u32]]) -> Vec<[f32; 4]> =
        frap_geom::mikktspace::corner_tangents;
    use frap_bmesh::ops::{decimate_collapse, decimate_dissolve, decimate_unsubdivide};
    let _: fn(&mut BMesh, &decimate_collapse::DecimateCollapseIn) =
        decimate_collapse::decimate_collapse;
    let _: fn(&mut BMesh, f32, bool, frap_bmesh::ops::dissolve_limit::Delimit) =
        decimate_dissolve::decimate_dissolve;
    let _: fn(&mut BMesh, i32) = decimate_unsubdivide::decimate_unsubdivide;
    let _: fn(
        &mut BMesh,
        bool,
        f32,
        bool,
        &mut Vec<(frap_bmesh::VertId, frap_bmesh::VertId)>,
    ) -> bool = frap_bmesh::ops::intersect_edges::intersect_edges;
}

/// M3 data and extension points: `LoweredMaterial` extension fields, multiple scenes,
/// textures, NODES inputs, modifier ID pointers, glTF material-extension and export slices.
fn m3_m4_data(main: &mut Main, win: &mut WindowState, log: &mut ChangeLog) {
    let lm = LoweredMaterial::gltf_default();
    let e: &MaterialExts = &lm.ext;
    let _: (
        &Option<frap_material::Clearcoat>,
        &Option<frap_material::Sheen>,
        &Option<frap_material::Transmission>,
        &Option<frap_material::Volume>,
        &Option<f32>,
        &Option<frap_material::Specular>,
        &Option<frap_material::Iridescence>,
        &Option<frap_material::Anisotropy>,
        &Option<f32>,
        &Option<frap_material::DiffuseTransmission>,
    ) = (
        &e.clearcoat,
        &e.sheen,
        &e.transmission,
        &e.volume,
        &e.ior,
        &e.specular,
        &e.iridescence,
        &e.anisotropy,
        &e.dispersion,
        &e.diffuse_transmission,
    );
    let _: f32 = lm.occlusion_strength;
    let _: fn(&mut Main, &str, &mut ChangeLog) -> IdRef<SceneId> = frap_scene::scenes::new_scene;
    let _: fn(
        &mut Main,
        IdRef<SceneId>,
        frap_scene::scenes::SceneCopy,
        &mut ChangeLog,
    ) -> IdRef<SceneId> = frap_scene::scenes::copy_scene;
    let _: fn(&mut Main, &mut WindowState, IdRef<SceneId>, &mut ChangeLog) -> Result<(), String> =
        frap_scene::scenes::remove_scene;
    let _: fn(&Main, &mut WindowState, IdRef<SceneId>) -> Result<(), String> =
        frap_scene::window::set_scene;
    let t = frap_scene::id::Texture::with_type("Tex", "IMAGE");
    let _: (
        &SmolStr,
        &frap_scene::generated::plain::TextureFields,
        Option<IdAny>,
    ) = (&t.r#type, &t.fields, t.fields.image);
    let n = frap_scene::generated::modifier_settings::NodesSettings::default();
    let _: &IndexMap<SmolStr, frap_scene::id::GnInput> = &n.inputs.0;
    let s = frap_scene::id::ModifierSettings::new(
        frap_scene::generated::modifier_settings::ModifierType::Boolean,
    );
    let _: SmallVec<[(&'static str, Option<IdAny>); 2]> = s.id_refs();
    let _ = (main, win, log);
    use frap_io_gltf::{ExtExportCx, ExtImportHost, MaterialExtExporter, MaterialExtImporter};
    let _: &[MaterialExtExporter] = &frap_io_gltf::MATERIAL_EXT_EXPORTERS;
    let _: &[MaterialExtImporter] = &frap_io_gltf::MATERIAL_EXT_IMPORTERS;
    let _ = |x: &MaterialExtExporter| -> fn(
        &mut ExtExportCx<'_, '_>,
        &LoweredMaterial,
        &mut frap_gltf::json::Material,
    ) -> Result<(), frap_io_gltf::ExportError> { x.run };
    let _ = |x: &MaterialExtImporter| -> fn(
        &mut dyn ExtImportHost,
        &frap_gltf::json::Material,
        &mut LoweredMaterial,
    ) -> Result<(), frap_io_gltf::import::ImportError> { x.run };
    let _: &[&'static [&'static str]] = &frap_io_gltf::export::SUPPORTED_EXPORT_PROPS;
    let _: fn(&str) -> bool = frap_io_gltf::export::is_supported;
    let st = frap_io_gltf::ExportSettings::default();
    let _: (&frap_spec_ops_io::export_scene::ExportSceneGltf, &Option<_>) =
        (&*st.props, &st.evaluated_meshes);
}

#[test]
fn m3_m4_contract_delta_cheap_parts_run() {
    // Defaults that WPs rely on.
    let lm = LoweredMaterial::gltf_default();
    assert_eq!(lm.ext, MaterialExts::default());
    assert!(lm.ext.clearcoat.is_none() && lm.ext.ior.is_none());
    assert_eq!(lm.occlusion_strength, 1.0);
    assert_eq!(frap_subdiv::SubdivSettings::default().level, 1);
    assert_eq!(
        frap_boolean::BoolOptions::default().solver,
        frap_boolean::SolverKind::Exact
    );
    assert_eq!(frap_uv::PackParams::default().margin, 0.001);
    assert!(frap_modifiers::registry::duplicates().is_empty());
    let t = frap_scene::id::Texture::new("Texture");
    assert_eq!(t.r#type, "IMAGE");
    let st = frap_io_gltf::ExportSettings::default();
    assert!(!st.props.export_apply.value && !st.props.export_cameras.value);
    assert!(frap_io_gltf::export::is_supported("export_format"));
    // Modifier ID pointers are visible to user counting and remapping.
    let mut s = frap_scene::id::ModifierSettings::new(
        frap_scene::generated::modifier_settings::ModifierType::Nodes,
    );
    assert_eq!(
        s.id_refs().iter().map(|(k, _)| *k).collect::<Vec<_>>(),
        ["node_group"]
    );
    assert!(s.set_id_ref("node_group", None));
    assert!(!s.set_id_ref("object", None));
    // The stack passes a mesh through untouched without modifiers (frap-modifiers stack walk).
    let _ = (
        modifier_contract as fn(&mut dyn frap_modifiers::ModEvalCx, &frap_modifiers::Modifier),
        backends as fn(),
        m3_m4_data as fn(&mut Main, &mut WindowState, &mut ChangeLog),
    );
}

// ---------------------------------------------------------------- M5+M6 W0 contract delta

/// M6 animation data model (`frap-anim`, WP 6.1) and evaluation signatures.
fn m6_anim() {
    use frap_anim::eval::{evaluate_action, evaluate_anim, evaluate_fcurve, recalc_handles};
    use frap_anim::{
        ActionGroup, ActionLayer, ActionLookup, ActionSlot, ActionStrip, AnimChannel, Channelbag,
        Easing, Extrapolation, FCurve, HandleType, Interpolation, Keyframe, KeyframeStrip,
        KeyframeType, NlaStrip, NlaTrack,
    };
    let _: fn(&FCurve, f32) -> f32 = evaluate_fcurve;
    let _: fn(&mut FCurve) = recalc_handles;
    let _: fn(&Action, i32, f32) -> Vec<AnimChannel> = evaluate_action;
    let _: fn(&AnimData, &dyn ActionLookup, f32) -> Vec<AnimChannel> = evaluate_anim;
    let _ = |a: &Action| {
        let _: (&[ActionSlot], &[ActionLayer], Option<&Channelbag>) =
            (&a.slots, &a.layers, a.channelbag(1));
    };
    let _ = |l: &ActionLayer| {
        let _: &[ActionStrip] = &l.strips;
    };
    let _ = |s: &KeyframeStrip| {
        let _: &[Channelbag] = &s.channelbags;
    };
    let _ = |c: &Channelbag| {
        let _: (i32, &[ActionGroup], &[FCurve]) = (c.slot_handle, &c.groups, &c.fcurves);
    };
    let _ = |f: &FCurve| {
        let _: (&SmolStr, i32, &[Keyframe], Extrapolation) = (
            &f.data_path,
            f.array_index,
            &f.keyframe_points,
            f.extrapolation,
        );
    };
    let _ = |k: &Keyframe| {
        let _: ([f32; 2], HandleType, Interpolation, Easing, KeyframeType) = (
            k.co,
            k.handle_left_type,
            k.interpolation,
            k.easing,
            k.r#type,
        );
    };
    let _ = |d: &AnimData| {
        let _: (&[NlaTrack], Option<IdRef<ActionId>>) = (&d.nla_tracks, d.action);
    };
    let _ = |t: &NlaTrack| {
        let _: &[NlaStrip] = &t.strips;
    };
}

/// M6 scene data: armatures, poses, edit bones, shape keys; evaluation hooks.
fn m6_scene() {
    use frap_scene::edit_armature::{EditArmature, EditBone};
    use frap_scene::id::{Armature, Bone, BoneCollection, Key, Pose, PoseBone, ShapeKey};
    let _ = |a: &Armature| {
        let _: (&[Bone], &[BoneCollection], Option<&Bone>) =
            (&a.bones, &a.collections, a.bone("Bone"));
    };
    let _ = |p: &Pose| {
        let _: &[PoseBone] = &p.bones;
    };
    let _: fn(&str) -> PoseBone = |n| PoseBone::new(n);
    let _ = |k: &Key| {
        let _: (&[ShapeKey], Option<IdRef<MeshId>>) = (&k.key_blocks, k.user);
    };
    let _ = |e: &EditArmature| {
        let _: &[EditBone] = &e.edit_bones;
    };
    let _ = |s: &EditState| {
        let _: &BTreeMap<IdKey, Arc<EditArmature>> = &s.armatures;
    };
    let _ = |o: &EvaluatedObject| {
        let _: Option<&Arc<frap_scene::EvaluatedPose>> = o.pose.as_ref();
    };
}

/// M5 render contract: engine support query, EEVEE settings, texture lookup, EEVEE module.
fn m5_render(s: &dyn RenderService, sc: &RenderScene, r: IdRef<ImageId>) {
    use frap_render_scene::{REeveeSettings, RLight, RLightEevee};
    let _: bool = s.supports_engine("BLENDER_EEVEE");
    let _: Option<&Arc<frap_imbuf::DecodedImage>> = sc.image(r);
    let _: &IndexMap<IdKey, frap_render_scene::ImageKey> = &sc.image_ids;
    let _: &REeveeSettings = &sc.settings.eevee;
    let _ = |l: &RLight| {
        let _: &RLightEevee = &l.eevee;
    };
    use frap_render::eevee::{EEVEE_FEATURES, Eevee, EeveeFeature, ibl, pipeline};
    let _: &[EeveeFeature] = &EEVEE_FEATURES;
    let _: fn() -> Eevee = Eevee::new;
    let _: bool = pipeline::IMPLEMENTED;
    let _: fn(&frap_render::Gpu, &RenderScene) -> Result<Option<ibl::WorldLighting>, RenderError> =
        ibl::world_lighting;
    let _ =
        |f: &EeveeFeature| -> fn(&LoweredMaterial, &RenderScene, &mut pipeline::MaterialParams) {
            f.pack
        };
    let _: &[frap_color::lut::BakedLut] = frap_color::agx::BAKED;
}

/// M6 glTF extension points.
fn m6_gltf() {
    use frap_io_gltf::export::node_map::NodeMap;
    let _: fn(&mut frap_io_gltf::export::gather::Gather) -> Result<(), frap_io_gltf::ExportError> =
        frap_io_gltf::export::animation::gather;
    let _ = |m: &NodeMap, s: IdRef<SceneId>, o: IdRef<ObjectId>| {
        let _: (Option<u32>, Option<u32>) = (m.object(s, o), m.bone(s, o, "Bone"));
    };
}

#[test]
fn m5_m6_contract_delta_cheap_parts_run() {
    let ad = AnimData::default();
    assert!(ad.use_nla && ad.action_influence == 1.0 && ad.action_blend_type == "REPLACE");
    let k = frap_anim::Keyframe::new(1.0, 2.0);
    assert_eq!(k.interpolation, frap_anim::Interpolation::Bezier);
    assert_eq!(k.handle_left_type, frap_anim::HandleType::AutoClamped);
    assert_eq!(
        frap_anim::Interpolation::from_identifier("ELASTIC"),
        Some(frap_anim::Interpolation::Elastic)
    );
    assert_eq!(Action::new("A").slots.len(), 0);
    assert_eq!(
        frap_scene::id::PoseBone::new("Bone").rotation_mode,
        "QUATERNION"
    );
    assert_eq!(
        frap_scene::id::Armature::new("Armature").display_type,
        "OCTAHEDRAL"
    );
    assert!(frap_scene::id::Key::new("Key").use_relative);
    let rs = frap_render_scene::RSettings::default();
    assert_eq!(rs.eevee.filter_size, 1.5);
    assert!(
        frap_render::eevee::features()
            .iter()
            .all(|f| !f.name.is_empty())
    );
    let _ = (
        m6_anim as fn(),
        m6_scene as fn(),
        m5_render as fn(&dyn RenderService, &RenderScene, IdRef<ImageId>),
        m6_gltf as fn(),
    );
}

// ---------------------------------------------------------------- M7 W0 (decision log M7W0)

/// Codec traits and backends (`frap-codec-mesh`, `frap-imbuf`).
fn m7_codecs() {
    use frap_codec_mesh::{
        CodecError, DracoBackend, DracoCodec, DracoEncoded, DracoQuantization, MeshData,
        MeshoptCodec, MeshoptFilter, MeshoptMode, MeshoptVersion,
    };
    let _ = |c: &dyn DracoCodec, m: &MeshData| -> Result<DracoEncoded, CodecError> {
        c.encode(m, &DracoQuantization::default(), 7)
    };
    let _ = |c: &dyn DracoCodec, b: &[u8]| -> Result<MeshData, CodecError> { c.decode(b, &[0, 1]) };
    let _ = |c: &dyn MeshoptCodec, d: &[u8]| -> Result<Vec<u8>, CodecError> {
        c.encode_vertex(d, 1, 12, MeshoptVersion::Khr)?;
        c.encode_index(&[0, 1, 2], 3, 2, MeshoptMode::Triangles)?;
        c.decode(d, 1, 12, MeshoptMode::Attributes, MeshoptFilter::Octahedral)
    };
    let _: fn(DracoBackend) -> Option<&'static dyn DracoCodec> = frap_codec_mesh::draco::codec;
    let _: fn() -> Option<&'static dyn MeshoptCodec> = frap_codec_mesh::meshopt::codec;
    let _: fn(&[u8]) -> Result<frap_imbuf::DecodedImage, frap_imbuf::ImageError> =
        frap_imbuf::ktx2::decode;
    let _: fn(
        &frap_imbuf::DecodedImage,
        frap_imbuf::basis::BasisMode,
    ) -> Result<Vec<u8>, frap_imbuf::ImageError> = frap_imbuf::basis::encode_ktx2;
}

/// glTF extension points (`frap-io-gltf`) and typed extensions (`frap-gltf`).
fn m7_gltf() {
    use frap_gltf::ext::{
        ext_mesh_gpu_instancing::ExtMeshGpuInstancing,
        ext_meshopt_compression::ExtMeshoptCompression,
        ext_texture_webp::ExtTextureWebp,
        khr_draco_mesh_compression::KhrDracoMeshCompression,
        khr_materials_diffuse_transmission::KhrMaterialsDiffuseTransmission,
        khr_materials_variants::{KhrMaterialsVariants, KhrMaterialsVariantsPrimitive},
        khr_mesh_quantization::KhrMeshQuantization,
        khr_meshopt_compression::KhrMeshoptCompression,
        khr_node_visibility::KhrNodeVisibility,
        khr_texture_basisu::KhrTextureBasisu,
    };
    fn named<E: frap_gltf::Extension>() -> &'static str {
        E::NAME
    }
    let _ = [
        named::<ExtMeshGpuInstancing>(),
        named::<ExtMeshoptCompression>(),
        named::<ExtTextureWebp>(),
        named::<KhrDracoMeshCompression>(),
        named::<KhrMaterialsDiffuseTransmission>(),
        named::<KhrMaterialsVariants>(),
        named::<KhrMaterialsVariantsPrimitive>(),
        named::<KhrMeshQuantization>(),
        named::<KhrMeshoptCompression>(),
        named::<KhrNodeVisibility>(),
        named::<KhrTextureBasisu>(),
    ];
    let d = frap_io_gltf::PrimitiveDraft::default();
    let e: &frap_io_gltf::export::mesh::ext::PrimitiveEncoding = &d.encoding;
    let _ = (
        &e.attributes,
        &e.targets,
        e.without_buffer_views,
        &e.extensions,
        &e.extensions_used,
    );
    let _ = |cx: &frap_io_gltf::MeshExportCx| cx.warn("message");
    let _ = |b: &mut frap_gltf::BufferBuilder, d: &frap_gltf::AccessorData| {
        b.push_accessor_without_view(d, &frap_gltf::AccessorOpts::default());
        b.push_indices_without_view(&[0, 1, 2]);
    };
    let _: fn(&frap_io_gltf::ExportSettings, frap_io_gltf::ExportOutput) -> _ =
        frap_io_gltf::export::gltfpack::apply;
}

#[test]
fn m7_contract_delta_cheap_parts_run() {
    let q = frap_codec_mesh::DracoQuantization::default();
    assert_eq!(
        (q.position, q.normal, q.texcoord, q.color, q.generic),
        (14, 10, 12, 10, 12),
        "the add-on's export_draco_*_quantization defaults"
    );
    assert_eq!(
        frap_codec_mesh::MeshoptFilter::default().identifier(),
        "NONE"
    );
    assert_eq!(
        frap_codec_mesh::MeshoptMode::Triangles.identifier(),
        "TRIANGLES"
    );
    assert!(frap_imbuf::ktx2::is_ktx2(&frap_imbuf::ktx2::IDENTIFIER));
    assert_eq!(
        frap_imbuf::FileFormat::detect(&frap_imbuf::ktx2::IDENTIFIER),
        Some(frap_imbuf::FileFormat::Ktx2)
    );
    let s = frap_scene::id::Scene::new("Scene", Vec::new());
    assert!(s.timeline_markers.is_empty() && s.gltf2_variants.variants.is_empty());
    assert_eq!(
        frap_scene::id::TimelineMarker::default().frame,
        0,
        "TimelineMarkers.new(frame=0)"
    );
    let m = frap_mesh::Mesh::new("Mesh");
    assert_eq!(m.gltf2_variants.materials().count(), 0);
    let d = frap_io_gltf::PrimitiveDraft::default();
    assert!(!d.encoding.without_buffer_views && d.encoding.extensions.is_empty());
    let _ = (m7_codecs as fn(), m7_gltf as fn());
}

// ------------------------------------------------------------------------- M8 W0 (decision log
// M8W0-4..9): the undo request, the operator gates, the planned rows, the BVH and the frap_scene
// variant operators.

/// `frap_geom::bvh` (WP 8.5; never called: the bodies are `todo!()` until then).
fn m8_bvh() {
    let mut t = frap_geom::bvh::BvhTree::new(4, 0.0, 2, 6);
    t.insert(0, &[[0.0; 3], [1.0; 3]]);
    t.balance();
    let _: Vec<(usize, usize)> = t.overlap(&t, &mut |_, _| true);
    let _: Option<frap_geom::bvh::Nearest> = t.find_nearest([0.0; 3], f32::MAX, &mut |_, _, _| {});
    let _: Option<frap_geom::bvh::RayHit> = t.ray_cast(
        [0.0; 3],
        [0.0, 0.0, 1.0],
        0.0,
        f32::MAX,
        &mut |_, _, _, _| {},
    );
    let _: usize = t.range_query([0.0; 3], 1.0, &mut |_, _, _| {});
}

/// The `frap_scene::gltf_variants` operator bodies (M8 W0, implemented).
fn m8_variants(main: &mut Main, log: &mut ChangeLog, ob: IdRef<ObjectId>) {
    use frap_scene::gltf_variants as v;
    v::variant_add(main, log);
    v::variant_remove(main, log);
    let _: Result<(), v::VariantOpError> = v::display_variant(main, log);
    let _: Result<(), v::VariantOpError> = v::assign_to_variant(main, ob, log);
    let _: Result<(), v::VariantOpError> = v::reset_to_original(main, ob, log);
    let _: Result<(), v::VariantOpError> = v::assign_as_original(main, ob, log);
    let _: Result<(), v::VariantOpError> = v::variants_slot_add(main, ob, log);
    let _: Result<bool, v::VariantOpError> = v::material_to_variant(main, ob, log);
    let _: Result<bool, v::VariantOpError> = v::remove_material_variant(main, ob, log);
}

#[test]
fn m8_contract_delta_cheap_parts_run() {
    frap_ops_all::link();
    // OPERATORS.md: M9 is the post-M8 backlog; the hint names the row (M8W0-3, M8W0-4).
    assert_eq!(frap_op::planned::operator_milestone("ed.undo"), "M8");
    assert_eq!(
        frap_op::planned::operator_milestone("object.voxel_remesh"),
        frap_op::planned::POST_M8
    );
    for (idname, planned, says) in [
        ("object.track_set", "M9", "after M8"),
        ("ed.undo", "M8", "plans it for M8"),
        ("anim.change_frame", "OUT", "does not list it"),
        ("mesh.knife_project", "OUT", "marks it OUT"),
        ("object.constraint_add", "OUT", "marks it OUT"),
    ] {
        let e = frap_op::lookup(idname).unwrap_err();
        assert_eq!(e.code, frap_op::errors::OP_NOT_IMPLEMENTED, "{idname}");
        assert_eq!(e.detail["planned"], planned, "{idname}");
        assert!(e.hint.as_deref().unwrap().contains(says), "{idname}: {e:?}");
    }
    // The glTF variant operators register a gate each (M8W0-6).
    let gated: BTreeSet<&str> = frap_op::OP_GATES.iter().map(|g| g.idname).collect();
    assert_eq!(gated.len(), 9);
    assert!(gated.iter().all(|i| i.starts_with("scene.gltf2_")));
    assert!(frap_op::gate("scene.gltf2_variant_add").is_some());
    assert!(frap_op::gate("scene.new").is_none());
    // The undo request (M8W0-5).
    let r = frap_op::UndoRequest::Push {
        message: "step".into(),
    };
    assert_ne!(r, frap_op::UndoRequest::Undo);
    let _ = (
        m8_bvh as fn(),
        m8_variants as fn(&mut Main, &mut ChangeLog, IdRef<ObjectId>),
    );
}
