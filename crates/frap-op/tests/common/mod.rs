// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared fixtures of the frap-op integration tests (WP 1.12): a `Host`/`Services` pair over
//! the factory startup, and hand-written dummy operator specs in a `dummy` namespace with
//! `OpProps` types (registration happens per test file, so each binary chooses its operators).
#![allow(dead_code, missing_docs)]

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use frap_imbuf::DecodedImage;
use frap_mesh::Mesh;
use frap_op::{ImageCache, OpError, PathResolver, Services};
use frap_render_scene::RenderService;
use frap_rna::{
    ArrayShape, Defaults, EnumItem, FloatStorage, IdKey, IdRef, ImageId, MeshId, ObjectId,
    OpOptions, OperatorSpec, PropDef, PropFlags, PropKind, Report, RnaError, Root, Subtype, Unit,
    Value,
};
use frap_scene::{
    ChangeLog, Context, EditState, EvalError, EvalMode, EvaluatedObject, Evaluator, Host, HostRead,
    Main, Preferences, Snapshot, WindowState,
};
use glam::Mat4;

// ------------------------------------------------------------------------------- host

/// Minimal evaluator: world matrix from location only, no mesh evaluation.
#[derive(Default)]
pub struct TestEval {
    pub calls: usize,
}

impl Evaluator for TestEval {
    fn object(
        &mut self,
        host: &dyn HostRead,
        ob: IdRef<ObjectId>,
        _mode: EvalMode,
    ) -> Result<Arc<EvaluatedObject>, EvalError> {
        self.calls += 1;
        let o = host.main().objects.get(ob).ok_or(EvalError {
            message: "no such object".into(),
        })?;
        Ok(Arc::new(EvaluatedObject {
            matrix_world: Mat4::from_translation(o.location.into()),
            mesh: None,
            bounds: None,
            pose: None,
            modifier_status: Vec::new(),
            warnings: Vec::new(),
        }))
    }
    fn depsgraph_update(&mut self, _host: &dyn HostRead) {}
}

#[derive(Clone)]
pub struct State {
    pub main: Main,
    pub win: WindowState,
    pub edit: EditState,
    pub prefs: Preferences,
}

impl HostRead for State {
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

pub struct TestHost {
    pub state: State,
    pub eval: TestEval,
    pub log: ChangeLog,
    pub reports: Vec<Report>,
}

impl TestHost {
    /// The factory startup (`frap_scene::startup::factory`).
    pub fn factory() -> TestHost {
        let (main, win) = frap_scene::startup::factory();
        TestHost {
            state: State {
                main,
                win,
                edit: EditState::default(),
                prefs: Preferences::default(),
            },
            eval: TestEval::default(),
            log: ChangeLog::default(),
            reports: Vec::new(),
        }
    }
    pub fn object(&self, name: &str) -> IdRef<ObjectId> {
        self.state.main.objects.find(name).expect("object exists")
    }
    pub fn object_names(&self) -> Vec<String> {
        self.state
            .main
            .objects
            .iter()
            .map(|(_, o)| o.header.name.to_string())
            .collect()
    }
    pub fn mesh_names(&self) -> Vec<String> {
        self.state
            .main
            .meshes
            .iter()
            .map(|(_, m)| m.header.name.to_string())
            .collect()
    }
    pub fn selected_names(&self) -> Vec<String> {
        let c = self.state.ctx();
        c.selected_objects()
            .into_iter()
            .map(|r| c.main.objects.get(r).unwrap().header.name.to_string())
            .collect()
    }
    pub fn active_name(&self) -> Option<String> {
        let c = self.state.ctx();
        c.active_object()
            .map(|r| c.main.objects.get(r).unwrap().header.name.to_string())
    }
    /// Makes `name` the active object of the context view layer.
    pub fn set_active(&mut self, name: Option<&str>) {
        let r = name.map(|n| self.object(n));
        let scene = self.state.win.scene;
        let vl_name = self.state.win.view_layer.clone();
        let s = self
            .state
            .main
            .scenes
            .get_mut(scene, &mut self.log)
            .unwrap();
        let vl = s
            .view_layers
            .iter_mut()
            .find(|v| v.name == vl_name)
            .unwrap();
        vl.active = r;
    }
    /// Selects exactly `names` in the context view layer.
    pub fn select_only(&mut self, names: &[&str]) {
        let refs: Vec<_> = names.iter().map(|n| self.object(n)).collect();
        let scene = self.state.win.scene;
        let s = self
            .state
            .main
            .scenes
            .get_mut(scene, &mut self.log)
            .unwrap();
        for vl in &mut s.view_layers {
            for b in &mut vl.bases {
                b.flag
                    .set(frap_scene::BaseFlag::SELECTED, refs.contains(&b.object));
            }
        }
    }
    pub fn object_mut(&mut self, name: &str) -> &mut frap_scene::id::Object {
        let r = self.object(name);
        self.state.main.objects.get_mut(r, &mut self.log).unwrap()
    }
    /// Removes an object from `bpy.data`, its collections and view layers (keeps its data).
    pub fn delete_object(&mut self, name: &str) {
        let r = self.object(name);
        let log = &mut self.log;
        let main = &mut self.state.main;
        let colls: Vec<_> = main.collections.iter().map(|(c, _)| c).collect();
        for c in colls {
            main.collections
                .get_mut(c, log)
                .unwrap()
                .objects
                .retain(|o| *o != r);
        }
        let scenes: Vec<_> = main.scenes.iter().map(|(s, _)| s).collect();
        for s in scenes {
            let s = main.scenes.get_mut(s, log).unwrap();
            s.collection.objects.retain(|o| *o != r);
            for vl in &mut s.view_layers {
                vl.bases.retain(|b| b.object != r);
                if vl.active == Some(r) {
                    vl.active = None;
                }
            }
        }
        main.objects.remove(r, log);
    }
}

impl HostRead for TestHost {
    fn main(&self) -> &Main {
        &self.state.main
    }
    fn win(&self) -> &WindowState {
        &self.state.win
    }
    fn edit(&self) -> &EditState {
        &self.state.edit
    }
    fn ctx(&self) -> Context<'_> {
        self.state.ctx()
    }
    fn mesh_view(&self, mesh: IdRef<MeshId>) -> Arc<Mesh> {
        self.state.mesh_view(mesh)
    }
}

impl Host for TestHost {
    fn main_mut(&mut self) -> (&mut Main, &mut ChangeLog) {
        (&mut self.state.main, &mut self.log)
    }
    fn edit_mut(&mut self) -> &mut EditState {
        &mut self.state.edit
    }
    fn split_edit(&mut self) -> (Context<'_>, &mut EditState) {
        let s = &mut self.state;
        (
            Context {
                main: &s.main,
                win: &s.win,
                prefs: &s.prefs,
            },
            &mut s.edit,
        )
    }
    fn win_mut(&mut self) -> &mut WindowState {
        &mut self.state.win
    }
    fn eval(&mut self) -> (&dyn HostRead, &mut dyn Evaluator) {
        (&self.state, &mut self.eval)
    }
    fn snapshot_at(&mut self, _frame: Option<i32>) -> Result<Snapshot, RnaError> {
        Ok(Snapshot {
            host: Box::new(self.state.clone()),
            eval: Box::new(TestEval::default()),
        })
    }
    fn mark_time_dirty(&mut self) {}
    fn apply_pending_animation(&mut self) -> Result<(), RnaError> {
        Ok(())
    }
    fn report(&mut self, r: Report) {
        self.reports.push(r);
    }
    fn set_path(&mut self, _: Root, _: &str, _: i32, _: Value) -> Result<(), RnaError> {
        Ok(())
    }
}

// ------------------------------------------------------------------------------- services

#[derive(Default)]
pub struct TestImages {
    pub map: HashMap<(IdKey, u64), Arc<DecodedImage>>,
}

impl ImageCache for TestImages {
    fn get(&self, image: IdRef<ImageId>, generation: u64) -> Option<Arc<DecodedImage>> {
        self.map.get(&(image.key(), generation)).cloned()
    }
    fn put(&mut self, image: IdRef<ImageId>, generation: u64, pixels: Arc<DecodedImage>) {
        self.map.insert((image.key(), generation), pixels);
    }
    fn evict(&mut self, key: IdKey) {
        self.map.retain(|(k, _), _| *k != key);
    }
}

pub struct TestServices {
    pub paths: PathResolver,
    pub images: TestImages,
    pub render_calls: usize,
}

impl Default for TestServices {
    fn default() -> Self {
        TestServices {
            paths: PathResolver {
                cwd: PathBuf::from("/work"),
                blend_dir: None,
            },
            images: TestImages::default(),
            render_calls: 0,
        }
    }
}

impl Services for TestServices {
    fn render(&mut self) -> Result<&mut dyn RenderService, OpError> {
        self.render_calls += 1;
        Err(OpError::Gpu("no GPU in frap-op tests".into()))
    }
    fn images(&mut self) -> &mut dyn ImageCache {
        &mut self.images
    }
    fn paths(&self) -> &PathResolver {
        &self.paths
    }
}

// ------------------------------------------------------------------------------- dummy specs

pub const fn item(identifier: &'static str, value: i32) -> EnumItem {
    EnumItem {
        identifier,
        name: identifier,
        description: "",
        value,
    }
}

pub const fn float(identifier: &'static str, default: f64, hard: (f64, f64)) -> PropDef {
    PropDef {
        identifier,
        name: identifier,
        description: "",
        kind: PropKind::Float {
            default: Defaults::Scalar(default),
            hard,
            soft: hard,
            precision: 3,
            storage: FloatStorage::F32,
        },
        subtype: Subtype::None,
        unit: Unit::None,
        array: ArrayShape::Scalar,
        flags: PropFlags::empty(),
        deprecated: None,
    }
}

pub const fn pointer(identifier: &'static str, fixed_type: &'static str) -> PropDef {
    PropDef {
        identifier,
        name: identifier,
        description: "",
        kind: PropKind::Pointer {
            fixed_type,
            is_id: false,
        },
        subtype: Subtype::None,
        unit: Unit::None,
        array: ArrayShape::Scalar,
        flags: PropFlags::empty(),
        deprecated: None,
    }
}

pub const fn int(identifier: &'static str, default: i64, hard: (i64, i64)) -> PropDef {
    PropDef {
        identifier,
        name: identifier,
        description: "",
        kind: PropKind::Int {
            default: Defaults::Scalar(default),
            hard,
            soft: hard,
        },
        subtype: Subtype::None,
        unit: Unit::None,
        array: ArrayShape::Scalar,
        flags: PropFlags::empty(),
        deprecated: None,
    }
}

pub const fn spec(
    idname: &'static str,
    bl_idname: &'static str,
    props: &'static [PropDef],
    macro_steps: &'static [&'static str],
) -> OperatorSpec {
    OperatorSpec {
        idname,
        bl_idname,
        label: idname,
        description: "frap-op test operator",
        options: OpOptions::REGISTER.union(OpOptions::UNDO),
        props,
        macro_steps,
        poll_matrix: None,
    }
}

/// Dummy operators (namespace `dummy`; `dummy::Add` ↔ `dummy::OPSPEC_DUMMY_OT_ADD`, the
/// codegen naming `#[operator]` infers).
pub mod dummy {
    use std::collections::BTreeSet;

    use frap_rna::{
        ArrayShape, Defaults, EnumDefault, FloatStorage, OpArgs, OpProps, OperatorSpec, Prop,
        PropDef, PropFlags, PropKind, Subtype, Unit,
    };
    use smol_str::SmolStr;

    use super::*;

    pub static MODE_ITEMS: [EnumItem; 3] = [item("A", 0), item("B", 1), item("C", 2)];
    pub static FLAG_ITEMS: [EnumItem; 2] = [item("X", 1), item("Y", 2)];

    pub static ADD_PROPS: [PropDef; 6] = [
        float("size", 1.0, (0.0, 100.0)),
        int("count", 3, (1, 10)),
        PropDef {
            identifier: "rotation",
            name: "Rotation",
            description: "",
            kind: PropKind::Float {
                default: Defaults::Array(&[0.0, 0.0, 0.0]),
                hard: (-1e12, 1e12),
                soft: (-6.0, 6.0),
                precision: 3,
                storage: FloatStorage::F32,
            },
            subtype: Subtype::Euler,
            unit: Unit::Rotation,
            array: ArrayShape::Fixed(3),
            flags: PropFlags::empty(),
            deprecated: None,
        },
        PropDef {
            identifier: "mode",
            name: "Mode",
            description: "",
            kind: PropKind::Enum {
                items: &MODE_ITEMS,
                default: EnumDefault::Item("A"),
                dynamic: Some("dummy_mode"),
            },
            subtype: Subtype::None,
            unit: Unit::None,
            array: ArrayShape::Scalar,
            flags: PropFlags::empty(),
            deprecated: None,
        },
        PropDef {
            identifier: "flags",
            name: "Flags",
            description: "",
            kind: PropKind::Enum {
                items: &FLAG_ITEMS,
                default: EnumDefault::Flags(&[]),
                dynamic: None,
            },
            subtype: Subtype::None,
            unit: Unit::None,
            array: ArrayShape::Scalar,
            flags: PropFlags::ENUM_FLAG,
            deprecated: None,
        },
        PropDef {
            identifier: "angle",
            name: "Angle",
            description: "",
            kind: PropKind::Float {
                default: Defaults::Scalar(0.0),
                hard: (-1e12, 1e12),
                soft: (-6.0, 6.0),
                precision: 3,
                storage: FloatStorage::F32,
            },
            subtype: Subtype::Angle,
            unit: Unit::Rotation,
            array: ArrayShape::Scalar,
            flags: PropFlags::empty(),
            deprecated: None,
        },
    ];
    pub static OPSPEC_DUMMY_OT_ADD: OperatorSpec =
        spec("dummy.add", "DUMMY_OT_add", &ADD_PROPS, &[]);

    #[derive(Debug, Clone)]
    pub struct Add {
        pub size: Prop<f32>,
        pub count: Prop<i32>,
        pub rotation: Prop<[f32; 3]>,
        pub mode: Prop<SmolStr>,
        pub flags: Prop<BTreeSet<SmolStr>>,
        pub angle: Prop<f32>,
    }
    impl OpProps for Add {
        const IDNAME: &'static str = "dummy.add";
        const BL_IDNAME: &'static str = "DUMMY_OT_add";
        fn spec() -> &'static OperatorSpec {
            &OPSPEC_DUMMY_OT_ADD
        }
        fn from_args(a: &OpArgs) -> Self {
            Add {
                size: a.prop(0, "size"),
                count: a.prop(1, "count"),
                rotation: a.prop(2, "rotation"),
                mode: a.prop(3, "mode"),
                flags: a.prop(4, "flags"),
                angle: a.prop(5, "angle"),
            }
        }
    }

    /// A props type with no props (`dummy.step_cancel`, `dummy.edit_only`).
    macro_rules! empty_props {
        ($ty:ident, $stat:ident, $id:literal, $bl:literal) => {
            pub static $stat: OperatorSpec = spec($id, $bl, &[], &[]);
            #[derive(Debug, Clone)]
            pub struct $ty;
            impl OpProps for $ty {
                const IDNAME: &'static str = $id;
                const BL_IDNAME: &'static str = $bl;
                fn spec() -> &'static OperatorSpec {
                    &$stat
                }
                fn from_args(_: &OpArgs) -> Self {
                    $ty
                }
            }
        };
    }
    empty_props!(
        StepCancel,
        OPSPEC_DUMMY_OT_STEP_CANCEL,
        "dummy.step_cancel",
        "DUMMY_OT_step_cancel"
    );
    empty_props!(
        EditOnly,
        OPSPEC_DUMMY_OT_EDIT_ONLY,
        "dummy.edit_only",
        "DUMMY_OT_edit_only"
    );
    empty_props!(Dup, OPSPEC_DUMMY_OT_DUP, "dummy.dup", "DUMMY_OT_dup");

    pub static STEP_A_PROPS: [PropDef; 1] = [int("v", 1, (-100, 100))];
    pub static OPSPEC_DUMMY_OT_STEP_A: OperatorSpec =
        spec("dummy.step_a", "DUMMY_OT_step_a", &STEP_A_PROPS, &[]);
    #[derive(Debug, Clone)]
    pub struct StepA {
        pub v: Prop<i32>,
    }
    impl OpProps for StepA {
        const IDNAME: &'static str = "dummy.step_a";
        const BL_IDNAME: &'static str = "DUMMY_OT_step_a";
        fn spec() -> &'static OperatorSpec {
            &OPSPEC_DUMMY_OT_STEP_A
        }
        fn from_args(a: &OpArgs) -> Self {
            StepA { v: a.prop(0, "v") }
        }
    }

    pub static STEP_B_PROPS: [PropDef; 1] = [PropDef {
        identifier: "value",
        name: "Value",
        description: "",
        kind: PropKind::Float {
            default: Defaults::Array(&[0.0, 0.0, 0.0]),
            hard: (-1e12, 1e12),
            soft: (-1e3, 1e3),
            precision: 3,
            storage: FloatStorage::F32,
        },
        subtype: Subtype::Translation,
        unit: Unit::Length,
        array: ArrayShape::Fixed(3),
        flags: PropFlags::empty(),
        deprecated: None,
    }];
    pub static OPSPEC_DUMMY_OT_STEP_B: OperatorSpec =
        spec("dummy.step_b", "DUMMY_OT_step_b", &STEP_B_PROPS, &[]);
    #[derive(Debug, Clone)]
    pub struct StepB {
        pub value: Prop<[f32; 3]>,
    }
    impl OpProps for StepB {
        const IDNAME: &'static str = "dummy.step_b";
        const BL_IDNAME: &'static str = "DUMMY_OT_step_b";
        fn spec() -> &'static OperatorSpec {
            &OPSPEC_DUMMY_OT_STEP_B
        }
        fn from_args(a: &OpArgs) -> Self {
            StepB {
                value: a.prop(0, "value"),
            }
        }
    }

    /// A macro props type: the nested args live in `OpArgs::nested`.
    macro_rules! macro_props {
        ($ty:ident, $stat:ident, $props:ident, $id:literal, $bl:literal, [$($step:literal),*]) => {
            pub static $props: [PropDef; [$($step),*].len()] = [$(pointer($step, $step)),*];
            pub static $stat: OperatorSpec = spec($id, $bl, &$props, &[$($step),*]);
            #[derive(Debug, Clone)]
            pub struct $ty;
            impl OpProps for $ty {
                const IDNAME: &'static str = $id;
                const BL_IDNAME: &'static str = $bl;
                fn spec() -> &'static OperatorSpec {
                    &$stat
                }
                fn from_args(_: &OpArgs) -> Self {
                    $ty
                }
            }
        };
    }
    macro_props!(
        MacroAb,
        OPSPEC_DUMMY_OT_MACRO_AB,
        MACRO_AB_PROPS,
        "dummy.macro_ab",
        "DUMMY_OT_macro_ab",
        ["DUMMY_OT_step_a", "DUMMY_OT_step_b"]
    );
    macro_props!(
        MacroCancel,
        OPSPEC_DUMMY_OT_MACRO_CANCEL,
        MACRO_CANCEL_PROPS,
        "dummy.macro_cancel",
        "DUMMY_OT_macro_cancel",
        ["DUMMY_OT_step_a", "DUMMY_OT_step_cancel", "DUMMY_OT_step_b"]
    );
    macro_props!(
        MacroCancelFirst,
        OPSPEC_DUMMY_OT_MACRO_CANCEL_FIRST,
        MACRO_CANCEL_FIRST_PROPS,
        "dummy.macro_cancel_first",
        "DUMMY_OT_macro_cancel_first",
        ["DUMMY_OT_step_cancel", "DUMMY_OT_step_a"]
    );
    macro_props!(
        MacroUnimpl,
        OPSPEC_DUMMY_OT_MACRO_UNIMPL,
        MACRO_UNIMPL_PROPS,
        "dummy.macro_unimpl",
        "DUMMY_OT_macro_unimpl",
        ["DUMMY_OT_step_a", "TRANSFORM_OT_translate"]
    );
}

/// `Input` keyword list from `(&str, Input)` pairs.
pub fn kw(pairs: Vec<(&str, frap_rna::Input)>) -> Vec<(String, frap_rna::Input)> {
    pairs.into_iter().map(|(k, v)| (k.to_owned(), v)).collect()
}

/// `(a, b, c)` of numbers.
pub fn tuple(v: &[f64]) -> frap_rna::Input {
    frap_rna::Input::Tuple(v.iter().map(|x| frap_rna::Input::Float(*x)).collect())
}

pub fn ints(v: &[i64]) -> frap_rna::Input {
    frap_rna::Input::Tuple(v.iter().map(|x| frap_rna::Input::Int(*x)).collect())
}
