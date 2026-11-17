// SPDX-License-Identifier: GPL-3.0-or-later
//! Invocation pipeline (§10.2) with dummy operators registered through `#[operator]` and
//! `linkme`: lookup (unknown + suggestions, `OP_NOT_IMPLEMENTED` for catalog operators),
//! positional rule, Blender's argument messages, macros in both spellings, dynamic enums,
//! `is_set`, the clamp report, `ANGLE_LIKELY_DEGREES`, poll-before-arguments, and the `OpMeta`
//! queries. Blender messages quoted here were verified on 5.2.4.

mod common;

use common::dummy;
use common::{TestHost, TestServices, ints, kw, tuple};
use frap_op::pipeline::{self, ANGLE_LIKELY_DEGREES, Invocation};
use frap_op::prelude::*;
use frap_op::{EnumItemOwned, OpOutput, PollReq, Registry, errors, operator};
use frap_rna::{FrapError, Input, PyExc};
use frap_scene::{Context, HostRead};
use indexmap::IndexMap;
use smol_str::SmolStr;

// ------------------------------------------------------------------------------- operators

fn mode_items(c: &Context) -> Vec<EnumItemOwned> {
    let mesh_active = c
        .active_object()
        .and_then(|r| c.main.objects.get(r))
        .is_some_and(|o| o.object_type() == "MESH");
    let ids: &[&str] = if mesh_active { &["A", "B"] } else { &["A"] };
    ids.iter()
        .map(|i| EnumItemOwned {
            identifier: (*i).into(),
            name: (*i).into(),
            description: SmolStr::default(),
        })
        .collect()
}

#[operator(poll = poll::always, tier = T0, dyn_enums = [("mode", mode_items)],
           notes = ["dummy operator of the frap-op tests"])]
fn add(cx: &mut OpCtx, p: dummy::Add) -> OpResult {
    cx.result("size", p.size.value);
    cx.result("size_is_set", p.size.is_set);
    cx.result("count", p.count.value);
    cx.result("count_is_set", p.count.is_set);
    cx.result("mode", Value::Str(p.mode.value));
    cx.result("flags", Value::EnumSet(p.flags.value));
    Ok(OpStatus::Finished)
}

#[operator(poll = poll::ALWAYS, tier = T0)]
fn step_a(cx: &mut OpCtx, p: dummy::StepA) -> OpResult {
    let scene = cx.ctx().win.scene;
    let (main, log) = cx.main_mut();
    main.scenes.get_mut(scene, log).unwrap().frame_current = 10 + p.v.value;
    cx.result("a", p.v.value);
    Ok(OpStatus::Finished)
}

#[operator(poll = poll::ALWAYS, tier = T0)]
fn step_b(cx: &mut OpCtx, p: dummy::StepB) -> OpResult {
    cx.result(
        "b",
        Value::Tuple(p.value.value.iter().map(|v| Value::Float(*v)).collect()),
    );
    Ok(OpStatus::Finished)
}

#[operator(poll = poll::ALWAYS, tier = T0)]
fn step_cancel(cx: &mut OpCtx, _p: dummy::StepCancel) -> OpResult {
    cx.report(ReportType::Warning, "step cancelled");
    Ok(OpStatus::Cancelled)
}

#[operator(poll = poll::ALWAYS, tier = T1, kind = Macro, support = Partial("test macro"))]
fn macro_ab(_cx: &mut OpCtx, _p: dummy::MacroAb) -> OpResult {
    Ok(OpStatus::Finished)
}

#[operator(poll = poll::ALWAYS, tier = T1, kind = Macro)]
fn macro_cancel(_cx: &mut OpCtx, _p: dummy::MacroCancel) -> OpResult {
    Ok(OpStatus::Finished)
}

#[operator(poll = poll::ALWAYS, tier = T1, kind = Macro)]
fn macro_unimpl(_cx: &mut OpCtx, _p: dummy::MacroUnimpl) -> OpResult {
    Ok(OpStatus::Finished)
}

#[operator(poll = poll::ALWAYS, tier = T1, kind = Macro)]
fn macro_cancel_first(_cx: &mut OpCtx, _p: dummy::MacroCancelFirst) -> OpResult {
    Ok(OpStatus::Finished)
}

// Hand-written registration (no `#[operator]`), as the M0 files still do.
fn edit_only_exec(_cx: &mut OpCtx, _a: &OpArgs) -> OpResult {
    Ok(OpStatus::Finished)
}
#[allow(unsafe_code)]
#[linkme::distributed_slice(OPERATORS)]
static EDIT_ONLY: OperatorDef = OperatorDef {
    spec: &dummy::OPSPEC_DUMMY_OT_EDIT_ONLY,
    poll: poll::EDITMESH,
    exec: edit_only_exec,
    kind: OpKind::Normal,
    dyn_enums: &[],
    tier: Tier::T0,
    support: Support::Full,
    notes: &[],
};

// ------------------------------------------------------------------------------- helpers

fn run(
    h: &mut TestHost,
    idname: &str,
    pos: Vec<Input>,
    args: Vec<(&str, Input)>,
) -> Result<Invocation, FrapError> {
    let mut s = TestServices::default();
    pipeline::invoke(h, &mut s, idname, pos, kw(args))
}

fn ok(h: &mut TestHost, idname: &str, args: Vec<(&str, Input)>) -> OpOutput {
    let inv = run(h, idname, vec![], args).expect("prepared");
    assert_eq!(
        inv.status.as_ref().ok(),
        Some(&OpStatus::Finished),
        "{idname}"
    );
    inv.output
}

fn err(h: &mut TestHost, idname: &str, args: Vec<(&str, Input)>) -> FrapError {
    run(h, idname, vec![], args).expect_err("must fail")
}

fn s(v: &str) -> Input {
    Input::Str(v.into())
}

fn detail<'a>(e: &'a FrapError, k: &str) -> &'a serde_json::Value {
    e.detail
        .get(k)
        .unwrap_or_else(|| panic!("detail.{k} missing in {e:?}"))
}

// ------------------------------------------------------------------------------- lookup

#[test]
fn registry_sees_dummy_ops_and_the_catalog() {
    let r = Registry::global();
    for id in [
        "dummy.add",
        "dummy.step_a",
        "dummy.macro_ab",
        "dummy.edit_only",
    ] {
        assert!(r.get(id).is_some(), "{id}");
    }
    assert_eq!(r.get("dummy.macro_ab").unwrap().kind, OpKind::Macro);
    assert!(r.known_idnames().contains(&"mesh.bevel"));
    assert!(r.idnames().iter().all(|i| i.starts_with("dummy.")));
}

#[test]
fn unknown_operator_with_suggestions() {
    let mut h = TestHost::factory();
    let e = err(&mut h, "dummy.ad", vec![]);
    assert_eq!(e.exception, PyExc::AttributeError);
    assert_eq!(e.code, errors::OP_UNKNOWN);
    assert_eq!(
        e.message,
        "Calling operator \"bpy.ops.dummy.ad\" error, could not be found"
    );
    assert_eq!(detail(&e, "did_you_mean")[0], "dummy.add");
    assert_eq!(e.hint.as_deref(), Some("did you mean bpy.ops.dummy.add?"));
    // Suggestions also come from operators frap does not implement.
    let e = err(&mut h, "mesh.bevl", vec![]);
    assert!(
        detail(&e, "did_you_mean")
            .as_array()
            .unwrap()
            .contains(&"mesh.bevel".into())
    );
}

#[test]
fn catalog_operator_without_implementation_is_not_implemented() {
    let mut h = TestHost::factory();
    let e = err(&mut h, "mesh.bevel", vec![("offset", Input::Float(0.1))]);
    assert_eq!(e.exception, PyExc::NotImplementedError);
    assert_eq!(e.code, errors::OP_NOT_IMPLEMENTED);
    assert_eq!(
        e.message,
        "mesh.bevel exists in Blender 5.2 but is not implemented by frap"
    );
    let sig = detail(&e, "signature").as_str().unwrap();
    assert!(
        sig.starts_with("bpy.ops.mesh.bevel(offset_type='OFFSET', offset=0.0, "),
        "{sig}"
    );
    // A typed (M1) operator whose implementation is not linked into this binary is known too.
    let e = err(&mut h, "mesh.primitive_cube_add", vec![]);
    assert_eq!(e.code, errors::OP_NOT_IMPLEMENTED);
    assert_eq!(
        detail(&e, "signature"),
        "bpy.ops.mesh.primitive_cube_add(size=2.0, calc_uvs=True, enter_editmode=False, \
         align='WORLD', location=(0.0, 0.0, 0.0), rotation=(0.0, 0.0, 0.0), scale=(0.0, 0.0, 0.0))"
    );
}

// ------------------------------------------------------------------------------- arguments

#[test]
fn unknown_keyword_wording() {
    let mut h = TestHost::factory();
    let e = err(&mut h, "dummy.add", vec![("sizee", Input::Int(2))]);
    assert_eq!(e.exception, PyExc::TypeError);
    assert_eq!(e.code, errors::OP_ARG_UNKNOWN);
    assert_eq!(
        e.message,
        "Converting py args to operator properties:: keyword \"sizee\" unrecognized"
    );
    assert_eq!(detail(&e, "operator"), "dummy.add");
    assert_eq!(detail(&e, "argument"), "sizee");
    assert_eq!(detail(&e, "did_you_mean"), &serde_json::json!(["size"]));
    assert_eq!(
        detail(&e, "valid"),
        &serde_json::json!(["size", "count", "rotation", "mode", "flags", "angle"])
    );
    // Wrong type and bad static enum: Blender's wording.
    let e = err(&mut h, "dummy.add", vec![("size", s("x"))]);
    assert_eq!(
        e.message,
        "Converting py args to operator properties: DUMMY_OT_add.size expected a float type, not str"
    );
    assert_eq!(e.code, errors::OP_ARG_TYPE);
    let e = err(
        &mut h,
        "dummy.add",
        vec![("flags", Input::Set(vec![s("Z")]))],
    );
    assert_eq!(e.code, errors::OP_ARG_ENUM);
}

#[test]
fn duplicate_keyword_is_a_syntax_error() {
    let mut h = TestHost::factory();
    let e = err(
        &mut h,
        "dummy.add",
        vec![("size", Input::Int(1)), ("size", Input::Int(2))],
    );
    assert_eq!(e.exception, PyExc::SyntaxError);
    assert_eq!(e.message, "keyword argument repeated: size");
}

#[test]
fn positional_rule() {
    let mut h = TestHost::factory();
    let inv = run(&mut h, "dummy.add", vec![s("EXEC_DEFAULT")], vec![]).unwrap();
    assert!(inv.output.reports.is_empty());
    let inv = run(&mut h, "dummy.add", vec![s("INVOKE_DEFAULT")], vec![]).unwrap();
    assert_eq!(inv.output.reports.len(), 1);
    assert_eq!(inv.output.reports[0].level, ReportType::Info);
    assert_eq!(
        inv.output.reports[0].message,
        "frap: INVOKE_DEFAULT runs as EXEC_DEFAULT (headless)"
    );
    for bad in [Input::Int(2), s("FOO"), Input::Float(1.0)] {
        let e = run(&mut h, "dummy.add", vec![bad], vec![]).unwrap_err();
        assert_eq!(e.exception, PyExc::TypeError);
        assert_eq!(e.code, errors::OP_ARG_POSITIONAL);
    }
}

#[test]
fn is_set_semantics() {
    let mut h = TestHost::factory();
    // Passing the default value still counts as set (`RNA_struct_property_is_set`).
    let out = ok(&mut h, "dummy.add", vec![("size", Input::Float(1.0))]);
    assert_eq!(out.result["size_is_set"], Value::Bool(true));
    assert_eq!(out.result["count_is_set"], Value::Bool(false));
    assert_eq!(out.result["count"], Value::Int(3));
    let c = pipeline::convert(
        Registry::global().get("dummy.add").unwrap(),
        &h.state.ctx(),
        kw(vec![("count", Input::Int(4))]),
    )
    .unwrap();
    assert_eq!(c.args.is_set.iter().collect::<Vec<_>>(), vec![1]);
    assert!(c.args.was_set("count") && !c.args.was_set("size"));
    // Spec order, defaults filled in.
    assert_eq!(
        c.args.values.keys().copied().collect::<Vec<_>>(),
        vec!["size", "count", "rotation", "mode", "flags", "angle"]
    );
}

#[test]
fn clamping_report_exact_wording() {
    let mut h = TestHost::factory();
    let out = ok(&mut h, "dummy.add", vec![("size", Input::Int(500))]);
    assert_eq!(out.result["size"], Value::Float(100.0));
    assert_eq!(out.reports.len(), 1);
    assert_eq!(out.reports[0].level, ReportType::Info);
    assert_eq!(
        out.reports[0].message,
        "frap: size=500 clamped to hard max 100.0"
    );
    let out = ok(&mut h, "dummy.add", vec![("size", Input::Int(-5))]);
    assert_eq!(
        out.reports[0].message,
        "frap: size=-5 clamped to hard min 0.0"
    );
    let out = ok(&mut h, "dummy.add", vec![("count", Input::Int(20))]);
    assert_eq!(
        out.reports[0].message,
        "frap: count=20 clamped to hard max 10"
    );
    let out = ok(
        &mut h,
        "dummy.add",
        vec![("rotation", tuple(&[1e16, 0.0, 0.0]))],
    );
    assert_eq!(
        out.reports[0].message,
        "frap: rotation=(1e+16, 0.0, 0.0) clamped to hard range [-1000000000000.0, 1000000000000.0]"
    );
}

#[test]
fn angle_likely_degrees_hint() {
    let mut h = TestHost::factory();
    let inv = run(
        &mut h,
        "dummy.add",
        vec![],
        vec![("rotation", ints(&[0, 0, 90]))],
    )
    .unwrap();
    assert_eq!(inv.hints.len(), 1);
    assert_eq!(inv.hints[0].code, ANGLE_LIKELY_DEGREES);
    assert_eq!(
        inv.hints[0].message,
        "rotation=0,0,90 is in radians (90 rad); did you mean radians(90)?"
    );
    assert_eq!(
        inv.hints[0].to_json(),
        serde_json::json!({"code": "ANGLE_LIKELY_DEGREES",
            "message": "rotation=0,0,90 is in radians (90 rad); did you mean radians(90)?"})
    );
    let inv = run(
        &mut h,
        "dummy.add",
        vec![],
        vec![("angle", Input::Float(180.0))],
    )
    .unwrap();
    assert_eq!(
        inv.hints[0].message,
        "angle=180.0 is in radians (180.0 rad); did you mean radians(180.0)?"
    );
    // Radians, small whole numbers and non-angle props give no hint.
    for (k, v) in [
        ("rotation", tuple(&[0.0, 0.0, 1.25])),
        ("angle", Input::Int(6)),
        ("angle", Input::Float(-3.0)),
        ("size", Input::Int(90)),
    ] {
        let inv = run(&mut h, "dummy.add", vec![], vec![(k, v)]).unwrap();
        assert!(inv.hints.is_empty(), "{k}");
    }
}

#[test]
fn dynamic_enums_follow_the_context() {
    let mut h = TestHost::factory();
    // Factory: Cube (mesh) active → items A, B.
    let out = ok(&mut h, "dummy.add", vec![("mode", s("B"))]);
    assert_eq!(out.result["mode"], Value::Str("B".into()));
    let e = err(&mut h, "dummy.add", vec![("mode", s("C"))]);
    assert_eq!(e.code, errors::OP_ARG_ENUM);
    assert_eq!(
        e.message,
        "Converting py args to operator properties: enum \"C\" not found in ('A', 'B')"
    );
    assert_eq!(detail(&e, "argument"), "mode");
    // Camera active → only A.
    h.set_active(Some("Camera"));
    let e = err(&mut h, "dummy.add", vec![("mode", s("B"))]);
    assert_eq!(
        e.message,
        "Converting py args to operator properties: enum \"B\" not found in ('A')"
    );
    // Without a context (legacy `convert_args`), the static superset applies.
    let spec = &dummy::OPSPEC_DUMMY_OT_ADD;
    assert!(frap_op::convert_args(spec, vec![], kw(vec![("mode", s("C"))])).is_ok());
}

#[test]
fn poll_runs_before_argument_conversion() {
    // Verified on 5.2.4: `bpy.ops.mesh.bevel(foo=1)` in Object Mode is a poll error.
    let mut h = TestHost::factory();
    let e = err(&mut h, "dummy.edit_only", vec![("foo", Input::Int(1))]);
    assert_eq!(e.exception, PyExc::RuntimeError);
    assert_eq!(e.code, errors::OP_POLL_FAILED);
    assert_eq!(
        e.message,
        "Operator bpy.ops.dummy.edit_only.poll() failed, context is incorrect"
    );
    assert_eq!(
        detail(&e, "requires"),
        &serde_json::json!(["MODE_EDIT_MESH"])
    );
    assert_eq!(detail(&e, "unmet"), &serde_json::json!(["MODE_EDIT_MESH"]));
    assert_eq!(
        e.hint.as_deref(),
        Some("bpy.ops.object.mode_set(mode='EDIT')")
    );
    // In Edit Mode the same call reaches argument conversion.
    h.object_mut("Cube").mode = frap_scene::ObjectMode::Edit;
    let e = err(&mut h, "dummy.edit_only", vec![("foo", Input::Int(1))]);
    assert_eq!(e.code, errors::OP_ARG_UNKNOWN);
}

// ------------------------------------------------------------------------------- macros

fn dict(pairs: Vec<(&str, Input)>) -> Input {
    Input::Dict(
        pairs
            .into_iter()
            .map(|(k, v)| (k.to_owned(), v))
            .collect::<IndexMap<_, _>>(),
    )
}

#[test]
fn macro_args_in_both_spellings() {
    let h = TestHost::factory();
    let def = Registry::global().get("dummy.macro_ab").unwrap();
    let bpy = pipeline::convert(
        def,
        &h.state.ctx(),
        kw(vec![
            ("DUMMY_OT_step_a", dict(vec![("v", Input::Int(5))])),
            ("DUMMY_OT_step_b", dict(vec![("value", ints(&[0, 0, 1]))])),
        ]),
    )
    .unwrap();
    let short = pipeline::convert(
        def,
        &h.state.ctx(),
        kw(vec![
            ("DUMMY_OT_step_a.v", Input::Int(5)),
            ("DUMMY_OT_step_b.value", ints(&[0, 0, 1])),
        ]),
    )
    .unwrap();
    assert_eq!(bpy.args, short.args);
    let a = &bpy.args.nested["DUMMY_OT_step_a"];
    assert_eq!(a.get("v"), &Value::Int(5));
    assert!(a.was_set("v"));
    assert!(bpy.args.was_set("DUMMY_OT_step_a"));
    // A step the caller did not mention gets its defaults.
    let only_a = pipeline::convert(
        def,
        &h.state.ctx(),
        kw(vec![("DUMMY_OT_step_a.v", Input::Int(2))]),
    )
    .unwrap();
    let b = &only_a.args.nested["DUMMY_OT_step_b"];
    assert!(!b.was_set("value") && !only_a.args.was_set("DUMMY_OT_step_b"));
}

#[test]
fn macro_runs_steps_in_order() {
    let mut h = TestHost::factory();
    let out = ok(
        &mut h,
        "dummy.macro_ab",
        vec![
            ("DUMMY_OT_step_a.v", Input::Int(5)),
            ("DUMMY_OT_step_b", dict(vec![("value", ints(&[0, 0, 1]))])),
        ],
    );
    assert_eq!(h.state.ctx().scene().frame_current, 15);
    assert_eq!(out.result["a"], Value::Int(5));
    assert_eq!(
        out.result["b"],
        Value::Tuple(vec![
            Value::Float(0.0),
            Value::Float(0.0),
            Value::Float(1.0)
        ])
    );
}

#[test]
fn macro_cancelled_step_stops_the_macro() {
    // wm_macro_exec / wm_macro_end (verified with a Python macro on 5.2.4): the macro stops at
    // the cancelled step and still finishes when an earlier step finished.
    let mut h = TestHost::factory();
    let out = ok(
        &mut h,
        "dummy.macro_cancel",
        vec![("DUMMY_OT_step_a.v", Input::Int(1))],
    );
    assert_eq!(h.state.ctx().scene().frame_current, 11);
    assert!(
        !out.result.contains_key("b"),
        "the step after the cancelled one never runs"
    );
    assert!(
        out.reports
            .iter()
            .any(|r| r.level == ReportType::Warning && r.message == "step cancelled")
    );
    let inv = run(&mut h, "dummy.macro_cancel_first", vec![], vec![]).unwrap();
    assert_eq!(inv.status.unwrap(), OpStatus::Cancelled);
}

#[test]
fn macro_argument_errors_use_blender_wording() {
    let mut h = TestHost::factory();
    let e = err(
        &mut h,
        "dummy.macro_ab",
        vec![("DUMMY_OT_step_a", Input::Int(5))],
    );
    assert_eq!(e.exception, PyExc::TypeError);
    assert_eq!(
        e.message,
        "Converting py args to operator properties: DUMMY_OT_macro_ab.DUMMY_OT_step_a expected a DUMMY_OT_step_a type, not int"
    );
    let e = err(
        &mut h,
        "dummy.macro_ab",
        vec![("DUMMY_OT_step_a.w", Input::Int(5))],
    );
    assert_eq!(
        e.message,
        "Converting py args to operator properties:: keyword \"w\" unrecognized"
    );
    assert_eq!(detail(&e, "argument"), "DUMMY_OT_step_a");
    assert_eq!(detail(&e, "sub_argument"), "w");
    let e = err(
        &mut h,
        "dummy.macro_ab",
        vec![("DUMMY_OT_step_a.v", s("x"))],
    );
    assert_eq!(
        e.message,
        "Converting py args to operator properties: DUMMY_OT_step_a.v expected an int type, not str"
    );
}

#[test]
fn real_blender_macro_conversion_matches_blender() {
    // `mesh.extrude_region_move` is a catalog macro; its conversion needs no implementation.
    // Messages verified on Blender 5.2.4.
    let spec = Registry::global().spec("mesh.extrude_region_move").unwrap();
    let conv = |args: Vec<(&str, Input)>| pipeline::convert_kwargs(spec, &[], None, kw(args));
    let e = conv(vec![(
        "TRANSFORM_OT_translate",
        dict(vec![("valuee", Input::Int(1))]),
    )])
    .unwrap_err();
    assert_eq!(
        e.message,
        "Converting py args to operator properties:: keyword \"valuee\" unrecognized"
    );
    let e = conv(vec![("TRANSFORM_OT_translate", Input::Int(5))]).unwrap_err();
    assert_eq!(
        e.message,
        "Converting py args to operator properties: MESH_OT_extrude_region_move.TRANSFORM_OT_translate expected a TRANSFORM_OT_translate type, not int"
    );
    let e = conv(vec![(
        "TRANSFORM_OT_translate",
        dict(vec![("value", ints(&[0, 0, 1, 2]))]),
    )])
    .unwrap_err();
    assert_eq!(e.exception, PyExc::ValueError);
    assert_eq!(
        e.message,
        "Converting py args to operator properties: sequences of dimension 0 should contain 3 items, not 4"
    );
    let e = conv(vec![(
        "TRANSFORM_OT_rotate",
        dict(vec![("value", Input::Int(1))]),
    )])
    .unwrap_err();
    assert_eq!(
        e.message,
        "Converting py args to operator properties:: keyword \"TRANSFORM_OT_rotate\" unrecognized"
    );
    let e = conv(vec![(
        "MESH_OT_extrude_region",
        dict(vec![("use_normal_flip", s("x"))]),
    )])
    .unwrap_err();
    assert_eq!(
        e.message,
        "Converting py args to operator properties: MESH_OT_extrude_region.use_normal_flip expected True/False or 0/1, not str"
    );
    let c = conv(vec![("TRANSFORM_OT_translate.value", ints(&[0, 0, 1]))]).unwrap();
    assert_eq!(
        c.args.nested["TRANSFORM_OT_translate"]
            .get("value")
            .as_vec3(),
        Some([0.0, 0.0, 1.0])
    );
    assert!(c.args.nested.contains_key("MESH_OT_extrude_region"));
}

#[test]
fn macro_with_unimplemented_step_is_not_implemented() {
    // `TRANSFORM_OT_translate` is a typed M1 spec whose implementation is not linked here.
    let mut h = TestHost::factory();
    let inv = run(
        &mut h,
        "dummy.macro_unimpl",
        vec![],
        vec![("TRANSFORM_OT_translate.value", ints(&[0, 0, 1]))],
    )
    .unwrap();
    match inv.status {
        Err(e @ OpError::NotImplemented(_)) => {
            let f = e.into_frap();
            assert_eq!(f.code, errors::OP_NOT_IMPLEMENTED);
            assert_eq!(
                f.message,
                "transform.translate (step of macro dummy.macro_unimpl) exists in Blender 5.2 but is not implemented by frap"
            );
        }
        other => panic!("{other:?}"),
    }
    // A catalog macro is not runnable at all.
    let e = err(&mut h, "mesh.extrude_region_move", vec![]);
    assert_eq!(e.code, errors::OP_NOT_IMPLEMENTED);
}

// ------------------------------------------------------------------------------- OpMeta

#[test]
fn op_meta_queries() {
    let mut h = TestHost::factory();
    let c = h.state.ctx();
    let a = pipeline::poll("dummy.edit_only", &c).unwrap();
    assert!(!a.ok);
    assert_eq!(a.requires, vec![PollReq::ModeEditMesh]);
    assert_eq!(a.unmet, vec![PollReq::ModeEditMesh]);
    assert_eq!(
        a.hint.as_deref(),
        Some("bpy.ops.object.mode_set(mode='EDIT')")
    );
    assert_eq!(a.detail()["unmet"], serde_json::json!(["MODE_EDIT_MESH"]));
    assert!(pipeline::poll("dummy.add", &c).unwrap().ok);
    // Not implemented: read from the dumped poll matrix (verified: False in Object Mode).
    let b = pipeline::poll("mesh.bevel", &c).unwrap();
    assert!(!b.ok);
    assert_eq!(b.estimated_from, Some("A"));
    assert!(pipeline::poll("object.select_all", &c).unwrap().ok);
    let e = pipeline::poll("mesh.foo", &c).unwrap_err();
    assert_eq!(e.exception, PyExc::AttributeError);
    assert_eq!(
        e.message,
        "Polling operator \"bpy.ops.mesh.foo\" error, could not be found"
    );
    // idname()/idname_py() are string transformations, also for unknown operators.
    assert_eq!(pipeline::idname_bl("mesh.foo"), "MESH_OT_foo");
    assert_eq!(pipeline::idname_bl("mesh.bevel"), "MESH_OT_bevel");
    assert_eq!(pipeline::idname_py("mesh.bevel"), "mesh.bevel");
    assert_eq!(pipeline::idname_py("MESH_OT_bevel"), "mesh.bevel");
    // get_rna_type()
    let e = pipeline::get_rna_type("mesh.foo").unwrap_err();
    assert_eq!(e.exception, PyExc::KeyError);
    assert_eq!(e.message, "get_rna_type(\"MESH_OT_foo\") not found");
    let bevel = pipeline::get_rna_type("mesh.bevel").unwrap().to_json();
    assert_eq!(bevel["identifier"], "MESH_OT_bevel");
    assert_eq!(bevel["frap"]["status"], "not_implemented");
    assert!(
        bevel["properties"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["identifier"] == "offset")
    );
    let add = pipeline::get_rna_type("dummy.add").unwrap().to_json();
    assert_eq!(add["frap"]["status"], "implemented");
    assert_eq!(add["frap"]["dynamic_enums"], serde_json::json!(["mode"]));
    assert_eq!(
        add["frap"]["notes"],
        serde_json::json!(["dummy operator of the frap-op tests"])
    );
    let rot = &add["properties"][2];
    assert_eq!(rot["unit"], "ROTATION (radians)");
    assert_eq!(rot["default"], serde_json::json!([0.0, 0.0, 0.0]));
    let m = pipeline::get_rna_type("dummy.macro_ab").unwrap().to_json();
    assert_eq!(
        m["frap"]["support"],
        serde_json::json!({"level": "partial", "note": "test macro"})
    );
    // Edit Mode: the edit-only poll passes; bevel's matrix column C says yes.
    h.object_mut("Cube").mode = frap_scene::ObjectMode::Edit;
    let c = h.state.ctx();
    assert!(pipeline::poll("dummy.edit_only", &c).unwrap().ok);
    let b = pipeline::poll("mesh.bevel", &c).unwrap();
    assert!(b.ok);
    assert_eq!(b.estimated_from, Some("C"));
}

#[test]
fn run_op_calls_another_operator() {
    let mut h = TestHost::factory();
    let mut s = TestServices::default();
    let spec = &dummy::OPSPEC_DUMMY_OT_ADD;
    let mut cx = OpCtx::new(&mut h, &mut s, spec);
    let args = pipeline::convert_kwargs(
        &dummy::OPSPEC_DUMMY_OT_STEP_A,
        &[],
        None,
        kw(vec![("v", Input::Int(3))]),
    )
    .unwrap()
    .args;
    assert_eq!(
        cx.run_op("dummy.step_a", args.clone()).unwrap(),
        OpStatus::Finished
    );
    assert_eq!(cx.ctx().scene().frame_current, 13);
    assert!(matches!(
        cx.run_op("mesh.bevel", args.clone()),
        Err(OpError::NotImplemented(_))
    ));
    assert!(matches!(
        cx.run_op("mesh.nope", args),
        Err(OpError::Runtime(_))
    ));
}
