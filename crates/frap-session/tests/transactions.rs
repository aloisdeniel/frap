// SPDX-License-Identifier: GPL-3.0-or-later
//! WP 1.14 acceptance: one statement is one transaction (§12). ERROR, CANCELLED and caught
//! panics restore the snapshot (`"state": "unchanged"`), with a proptest over random statement
//! sequences; `--max-elements` rolls back with `LIMIT_EXCEEDED`; the render service is created
//! lazily and answers `GPU_UNAVAILABLE` without the `gpu` feature.

mod support;

use frap_op::prelude::*;
use frap_rna::{OpOptions, OperatorSpec, PyExc, RnaError, RnaErrorKind, RnaPtr};
use frap_scene::{FuncArgs, Host, RnaFuncReg};
use proptest::prelude::*;
use serde_json::json;
use support::*;

// ------------------------------------------------------------------ test operators

fn mutate(cx: &mut OpCtx) -> Result<(), OpError> {
    let ob = cx
        .ctx()
        .active_object()
        .ok_or_else(|| OpError::Runtime("no active object".into()))?;
    cx.object_mut(ob)?.location = [5.0, 6.0, 7.0];
    Ok(())
}

fn exec_fail(cx: &mut OpCtx, _: &OpArgs) -> OpResult {
    mutate(cx)?;
    cx.report(ReportType::Warning, "about to fail");
    Err(OpError::Runtime("boom".into()))
}

fn exec_cancel(cx: &mut OpCtx, _: &OpArgs) -> OpResult {
    mutate(cx)?;
    cx.report(ReportType::Warning, "Active object is not a selected mesh");
    Ok(OpStatus::Cancelled)
}

fn exec_panic(cx: &mut OpCtx, _: &OpArgs) -> OpResult {
    mutate(cx)?;
    panic!("test operator panicked");
}

fn exec_render(cx: &mut OpCtx, _: &OpArgs) -> OpResult {
    cx.services().render()?;
    Ok(OpStatus::Finished)
}

fn exec_move(cx: &mut OpCtx, _: &OpArgs) -> OpResult {
    mutate(cx)?;
    Ok(OpStatus::Finished)
}

/// Specs of the test operators (`OperatorDef.spec` is `&'static`).
struct Specs {
    fail: OperatorSpec,
    cancel: OperatorSpec,
    panic: OperatorSpec,
    render: OperatorSpec,
    moved: OperatorSpec,
}

macro_rules! op_spec {
    ($idname:literal, $bl:literal) => {
        OperatorSpec {
            idname: $idname,
            bl_idname: $bl,
            label: "",
            description: "",
            options: OpOptions::empty(),
            props: &[],
            macro_steps: &[],
            poll_matrix: None,
        }
    };
}

static SPECS: Specs = Specs {
    fail: op_spec!(
        "frap_test.mutate_then_fail",
        "FRAP_TEST_OT_mutate_then_fail"
    ),
    cancel: op_spec!(
        "frap_test.mutate_then_cancel",
        "FRAP_TEST_OT_mutate_then_cancel"
    ),
    panic: op_spec!(
        "frap_test.mutate_then_panic",
        "FRAP_TEST_OT_mutate_then_panic"
    ),
    render: op_spec!("frap_test.render_probe", "FRAP_TEST_OT_render_probe"),
    moved: op_spec!("frap_test.move", "FRAP_TEST_OT_move"),
};

macro_rules! register {
    ($def:ident, $field:ident, $exec:ident) => {
        #[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
        #[frap_op::linkme::distributed_slice(OPERATORS)]
        #[linkme(crate = frap_op::linkme)]
        static $def: OperatorDef = OperatorDef {
            spec: &SPECS.$field,
            poll: poll::ALWAYS,
            exec: $exec,
            kind: OpKind::Normal,
            dyn_enums: &[],
            tier: Tier::T3,
            support: Support::Full,
            notes: &[],
        };
    };
}

register!(OP_FAIL, fail, exec_fail);
register!(OP_CANCEL, cancel, exec_cancel);
register!(OP_PANIC, panic, exec_panic);
register!(OP_RENDER, render, exec_render);
register!(OP_MOVE, moved, exec_move);

// ------------------------------------------------------------------ test RNA functions

fn move_cube(host: &mut dyn Host, this: &RnaPtr) -> Result<(), RnaError> {
    let frap_rna::Root::Id(id) = this.root else {
        return Err(RnaError::new(
            RnaErrorKind::Other,
            PyExc::RuntimeError,
            "not an ID",
        ));
    };
    let (main, log) = host.main_mut();
    let ob = id
        .typed()
        .and_then(|o| main.objects.get_mut(o, log))
        .ok_or_else(|| RnaError::new(RnaErrorKind::Other, PyExc::RuntimeError, "no object"))?;
    ob.location = [9.0, 9.0, 9.0];
    Ok(())
}

/// `Object.crazyspace_eval_clear()` stand-in: mutates, then fails.
fn func_fail(
    host: &mut dyn Host,
    this: &RnaPtr,
    _: &mut FuncArgs,
) -> Result<frap_rna::Value, RnaError> {
    move_cube(host, this)?;
    Err(RnaError::new(
        RnaErrorKind::Other,
        PyExc::RuntimeError,
        "Error: test function failed",
    ))
}

/// `Object.cache_release()` stand-in: mutates, then panics.
fn func_panic(
    host: &mut dyn Host,
    this: &RnaPtr,
    _: &mut FuncArgs,
) -> Result<frap_rna::Value, RnaError> {
    move_cube(host, this)?;
    panic!("test function panicked");
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[frap_rna::linkme::distributed_slice(frap_scene::RNA_FUNCS)]
#[linkme(crate = frap_rna::linkme)]
static FUNC_FAIL: RnaFuncReg = RnaFuncReg {
    srna: "Object",
    func: "crazyspace_eval_clear",
    imp: func_fail,
};

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[frap_rna::linkme::distributed_slice(frap_scene::RNA_FUNCS)]
#[linkme(crate = frap_rna::linkme)]
static FUNC_PANIC: RnaFuncReg = RnaFuncReg {
    srna: "Object",
    func: "cache_release",
    imp: func_panic,
};

fn quiet_panics() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| std::panic::set_hook(Box::new(|_| {})));
}

// ------------------------------------------------------------------ tests

#[test]
fn error_cancel_and_panic_restore_the_snapshot() {
    quiet_panics();
    let mut s = session();
    let before = s.state_dump();
    let r = one(&mut s, "bpy.ops.frap_test.mutate_then_fail()");
    assert_eq!(r["status"], "ERROR");
    assert_eq!(r["state"], "unchanged");
    assert_eq!(r["error"]["code"], "OP_EXEC_ERROR");
    assert_eq!(r["error"]["message"], "Error: boom");
    assert_eq!(r["reports"][0]["message"], "about to fail", "{r}");
    assert_eq!(s.state_dump(), before);

    let r = one(&mut s, "bpy.ops.frap_test.mutate_then_cancel()");
    assert_eq!(r["status"], "CANCELLED");
    assert_eq!(r["ok"], false);
    assert_eq!(r["state"], "unchanged");
    assert_eq!(
        r["reports"],
        json!([{"type": "WARNING", "message": "Active object is not a selected mesh"}])
    );
    assert!(r.get("changes").is_none());
    assert_eq!(s.state_dump(), before);

    let r = one(&mut s, "bpy.ops.frap_test.mutate_then_panic()");
    assert_eq!(r["status"], "ERROR");
    assert_eq!(r["error"]["code"], "INTERNAL");
    assert_eq!(r["error"]["exception"], "RuntimeError");
    assert!(
        r["error"]["message"]
            .as_str()
            .unwrap()
            .contains("test operator panicked")
    );
    assert_eq!(s.state_dump(), before);

    for src in [
        "bpy.data.objects['Cube'].crazyspace_eval_clear()",
        "bpy.data.objects['Cube'].cache_release()",
        // A nested call that fails inside an operator's arguments.
        "bpy.ops.mesh.primitive_cube_add(size=bpy.data.objects['Cube'].crazyspace_eval_clear())",
        "bpy.data.objects['Cube'].location = bpy.data.objects['Cube'].cache_release()",
    ] {
        let r = one(&mut s, src);
        assert_eq!(r["status"], "ERROR", "{src}: {r}");
        assert_eq!(s.state_dump(), before, "{src}");
    }
    let r = one(&mut s, "bpy.data.objects['Cube'].crazyspace_eval_clear()");
    assert_eq!(r["error"]["code"], "RNA_RUNTIME");
    assert_eq!(r["error"]["message"], "Error: test function failed");

    // The stream continues, and a committing statement after the failures commits.
    let r = one(&mut s, "bpy.ops.frap_test.move()");
    assert_eq!(r["status"], "FINISHED");
    assert_eq!(
        r["changes"]["modified"],
        json!(["bpy.data.objects['Cube']"])
    );
    assert_eq!(
        val(&mut s, "bpy.data.objects['Cube'].location"),
        json!([5, 6, 7])
    );
}

#[test]
fn limit_exceeded_rolls_back() {
    let mut s = session_with(|o| o.limits.max_elements = 10);
    let before = s.state_dump();
    let r = one(&mut s, "bpy.ops.mesh.primitive_cube_add()");
    assert_eq!(r["status"], "ERROR", "{r}");
    assert_eq!(r["error"]["code"], "LIMIT_EXCEEDED");
    assert_eq!(
        r["error"]["message"],
        "Error: frap: limit exceeded (max_elements)"
    );
    assert_eq!(s.state_dump(), before);
}

#[test]
fn render_service_is_created_lazily() {
    let mut s = session();
    assert!(!s.render_service_started());
    let r = one(&mut s, "bpy.ops.frap_test.render_probe()");
    if cfg!(feature = "gpu") {
        assert!(r["status"] == "FINISHED" || r["error"]["code"] == "GPU_UNAVAILABLE");
    } else {
        assert_eq!(r["error"]["code"], "GPU_UNAVAILABLE", "{r}");
        assert!(!s.render_service_started());
    }
}

/// Statements of the proptest: (source, expected to commit).
const POOL: &[(&str, bool)] = &[
    ("bpy.data.objects['Cube'].location", false),
    ("bpy.data.objects", false),
    ("print(bpy.data.objects['Cube'].location * 2)", false),
    ("bpy.ops.frap_test.mutate_then_fail.poll()", false),
    ("bpy.data.meshes['Cube'].vertices.foreach_get('co')", false),
    ("bpy.ops.frap_test.mutate_then_fail()", false),
    ("bpy.ops.frap_test.mutate_then_cancel()", false),
    ("bpy.ops.frap_test.mutate_then_panic()", false),
    ("bpy.data.objects['Cube'].crazyspace_eval_clear()", false),
    ("bpy.data.objects['Cube'].cache_release()", false),
    ("bpy.data.objects['Cube'].location = (1, 2)", false),
    ("bpy.data.objects['Cube'].location += (1, 1, 1)", false),
    ("bpy.data.objects.get('Cube').location = 'x'", false),
    (
        "bpy.data.meshes['Cube'].vertices.foreach_set('co', [1, 2, 3])",
        false,
    ),
    ("bpy.ops.mesh.primitive_cube_add(size='x')", false),
    ("bpy.ops.mesh.foo()", false),
    ("bpy.data.objects['Cube'].location.x = 1 / 0", false),
    ("bpy.ops.mesh.primitive_cube_add(", false),
    ("bpy.data.objects['Cube'].location = (1, 2, 3)", true),
    ("bpy.ops.mesh.primitive_cube_add()", true),
    ("bpy.data.meshes['Cube'].vertices[0].co = (0, 0, 1)", true),
    ("bpy.data.objects['Cube'].location.x += 1", true),
    ("bpy.ops.frap_test.move()", true),
];

proptest! {
    #![proptest_config(ProptestConfig { cases: 48, ..ProptestConfig::default() })]

    /// Any statement that does not finish leaves the state bit-identical; reads never change
    /// it; committing statements do (and the session keeps answering).
    #[test]
    fn failed_statements_leave_the_state_unchanged(picks in proptest::collection::vec(0..POOL.len(), 1..14)) {
        quiet_panics();
        let mut s = session();
        for i in picks {
            let (src, commits) = POOL[i];
            let before = s.state_dump();
            let rs: Vec<_> = s.exec_source(src).iter().map(|r| r.to_json()).collect();
            prop_assert_eq!(rs.len(), 1);
            let r = &rs[0];
            if r["status"] == "FINISHED" {
                prop_assert!(commits || r["kind"] != "op", "{} unexpectedly finished", src);
                if !commits {
                    prop_assert_eq!(s.state_dump(), before, "read {} changed the state", src);
                }
            } else {
                prop_assert!(!commits, "{} failed: {}", src, r);
                prop_assert_eq!(&r["state"], "unchanged");
                prop_assert_eq!(s.state_dump(), before, "{} left changes behind", src);
            }
        }
    }
}
