// SPDX-License-Identifier: GPL-3.0-or-later
//! M2 stabilization round 2: the protocol side of the exit verifier's findings that goldens
//! do not see (hints, codes of operators frap plans but has not implemented). The Blender-facing
//! messages are unchanged; only frap's hints are pinned here.

mod support;

use support::*;

#[test]
fn ed_undo_and_redo_are_planned_not_unknown() {
    let mut s = session();
    // M8 (WP 8.4) implements the four; with nothing to undo their polls fail as Blender's.
    for op in ["undo", "redo", "undo_redo"] {
        let (exc, code, msg) = err(&mut s, &format!("bpy.ops.ed.{op}()"));
        assert_eq!(
            (exc.as_str(), code.as_str()),
            ("RuntimeError", "OP_POLL_FAILED"),
            "ed.{op}"
        );
        assert_eq!(
            msg,
            format!("Operator bpy.ops.ed.{op}.poll() failed, context is incorrect")
        );
    }
    // The rest of the dumped `ed` namespace is still known but not implemented.
    let (exc, code, msg) = err(&mut s, "bpy.ops.ed.undo_history(item=0)");
    assert_eq!(
        (exc.as_str(), code.as_str()),
        ("NotImplementedError", "OP_NOT_IMPLEMENTED")
    );
    assert_eq!(
        msg,
        "ed.undo_history exists in Blender 5.2 but is not implemented by frap"
    );
    // Not a curated namespace: still unknown.
    let (_, code, _) = err(&mut s, "bpy.ops.nope.undo()");
    assert_eq!(code, "OP_UNKNOWN");
}

#[test]
fn variable_length_arrays_hint_a_list() {
    let mut s = session();
    one(&mut s, "bpy.ops.object.mode_set(mode='EDIT')");
    for src in [
        "frap.select_elements type=FACE indices=5",
        "bpy.ops.frap.select_elements(type='FACE', indices=(5))",
        "bpy.ops.frap.select_elements(type='FACE', indices=5)",
    ] {
        let r = one(&mut s, src);
        assert_eq!(r["error"]["code"], "OP_ARG_TYPE", "{src}: {r}");
        assert_eq!(
            r["error"]["hint"], "indices expects a list, e.g. [5] or [4, 6]",
            "{src}: {r}"
        );
    }
    // Fixed-length arrays keep the sized hint.
    let r = one(&mut s, "bpy.ops.mesh.primitive_cube_add(location=5)");
    assert_eq!(
        r["error"]["hint"],
        "location expects a sequence of 3 numbers, e.g. (0, 0, 1)"
    );
}

#[test]
fn short_form_syntax_hints_say_what_is_wrong() {
    let mut s = session();
    let hint = |s: &mut frap_session::Session, src: &str| {
        let r = one(s, src);
        assert_eq!(r["error"]["code"], "SYNTAX", "{src}: {r}");
        assert_eq!(r["error"]["message"], "invalid syntax", "{src}: {r}");
        r["error"]["hint"].as_str().unwrap_or_default().to_owned()
    };
    for src in [
        "frap.select_elements type=FACE indices=5,",
        "mesh.primitive_cube_add location=5,",
    ] {
        assert_eq!(
            hint(&mut s, src),
            "a short-form value cannot end with a comma: write the items as 0,0,1, or one \
             item of a list as [5]",
            "{src}"
        );
    }
    assert_eq!(
        hint(&mut s, "mesh.primitive_cube_add location=5, 0"),
        "no spaces inside a short-form tuple: 0,0,1"
    );
    assert!(
        hint(&mut s, "mesh.primitive_cube_add location=5, size=2")
            .starts_with("a short-form value cannot end")
    );
    for src in [
        "render.render write_still=True filepath=/tmp/x.png",
        "wm.save_as_mainfile filepath=~/x.frap",
        "wm.save_as_mainfile filepath=./x.frap",
        "wm.save_as_mainfile filepath=//x.frap",
    ] {
        assert_eq!(
            hint(&mut s, src),
            "quote paths and other text in single quotes, e.g. filepath='/tmp/out.png'",
            "{src}"
        );
    }
    // Values that start like numbers still parse.
    for src in [
        "mesh.primitive_cube_add location=.5,0,0",
        "mesh.primitive_cube_add location=-1,0,0 size=3",
    ] {
        assert_eq!(one(&mut s, src)["status"], "FINISHED", "{src}");
    }
}

#[test]
fn auto_merge_with_split_works() {
    // WP 4.6 implemented Auto Merge with split (EDBM_automerge_and_split): both requests,
    // refused with OP_NOT_IMPLEMENTED in M2, now run (goldens `edit/automerge_split/*`).
    let mut s = session();
    one(&mut s, "bpy.ops.object.mode_set(mode='EDIT')");
    for src in [
        "bpy.ops.transform.translate(value=(0, 0, 1), use_automerge_and_split=True)",
        "bpy.ops.mesh.extrude_manifold(TRANSFORM_OT_translate={\"value\": (0, 0, -0.5)})",
    ] {
        let r = one(&mut s, src);
        assert_eq!(r["status"], "FINISHED", "{src}: {r}");
    }
    // Auto Merge without split works: moving a vertex onto another welds them.
    let mut s = session();
    one(&mut s, "bpy.ops.object.mode_set(mode='EDIT')");
    one(
        &mut s,
        "bpy.context.scene.tool_settings.use_mesh_automerge = True",
    );
    one(&mut s, "bpy.ops.mesh.select_all(action='DESELECT')");
    one(
        &mut s,
        "bpy.ops.frap.select_elements(type='VERT', indices=[1])",
    );
    let r = one(&mut s, "bpy.ops.transform.translate(value=(0, 0, 2))");
    assert_eq!(r["status"], "FINISHED", "{r}");
    assert_eq!(r["edit_mesh"]["Cube"]["verts"], 7, "{r}");
}
