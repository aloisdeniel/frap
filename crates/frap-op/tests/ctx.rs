// SPDX-License-Identifier: GPL-3.0-or-later
//! `OpCtx` borrow shapes (RFC 0001 F-OP-5..8): `split_edit` (a `Context` and `&mut EditState`
//! at once), `split_eval` (a `&dyn HostRead` and `&mut dyn Evaluator` at once), `services`,
//! plus the other accessors, `snapshot_at` and `finish`.

mod common;

use std::sync::Arc;

use common::{TestHost, TestServices, dummy};
use frap_bmesh::BMesh;
use frap_imbuf::{DecodedImage, PixelData};
use frap_op::prelude::*;
use frap_rna::{IdRef, ImageId, MeshId};
use frap_scene::{EditMesh, EvalMode, ViewCache};

fn spec() -> &'static frap_rna::OperatorSpec {
    &dummy::OPSPEC_DUMMY_OT_ADD
}

#[test]
fn split_edit_reads_context_while_editing() {
    let mut h = TestHost::factory();
    let mut s = TestServices::default();
    let me: IdRef<MeshId> = h.state.main.meshes.find("Cube").unwrap();
    let mut cx = OpCtx::new(&mut h, &mut s, spec());
    {
        let (ctx, edit) = cx.split_edit();
        // Both borrows live together: read the cursor and tool settings, write the edit state.
        let cursor = ctx.cursor().location;
        let active = ctx.active_object();
        edit.meshes.insert(
            me.key(),
            EditMesh {
                bm: Arc::new(BMesh::default()),
                bm_gen: 1,
                view: ViewCache::default(),
            },
        );
        assert_eq!(cursor, [0.0; 3]);
        assert!(active.is_some());
        let _ = ctx.tool_settings();
    }
    drop(cx);
    assert_eq!(h.state.edit.meshes[&me.key()].bm_gen, 1);
}

#[test]
fn split_eval_reads_state_while_evaluating() {
    let mut h = TestHost::factory();
    let mut s = TestServices::default();
    let cube = h.object("Cube");
    let mut cx = OpCtx::new(&mut h, &mut s, spec());
    {
        let (host, eval) = cx.split_eval();
        let ctx = host.ctx();
        let selected = ctx.selected_objects();
        let ev = eval.object(host, cube, EvalMode::Viewport).unwrap();
        assert_eq!(selected, vec![cube]);
        assert_eq!(
            ev.matrix_world.w_axis.truncate().to_array(),
            host.main().objects.get(cube).unwrap().location
        );
    }
    let ev = cx.evaluated(cube).unwrap();
    assert!(ev.mesh.is_none());
    drop(cx);
    assert_eq!(h.eval.calls, 2);
}

#[test]
fn services_are_reachable_while_the_host_is_borrowed() {
    let mut h = TestHost::factory();
    let mut s = TestServices::default();
    let mut cx = OpCtx::new(&mut h, &mut s, spec());
    let img: IdRef<ImageId> = IdRef::from_key(frap_rna::IdKey::next());
    let pixels = Arc::new(DecodedImage {
        width: 1,
        height: 1,
        channels: 4,
        pixels: PixelData::U8(vec![1, 2, 3, 4]),
    });
    let ob = cx.ctx().active_object().unwrap();
    {
        let services = cx.services();
        assert_eq!(
            services.paths().resolve("//out/a.png"),
            std::path::PathBuf::from("/work/out/a.png")
        );
        assert_eq!(
            services.paths().resolve("out.glb"),
            std::path::PathBuf::from("/work/out.glb")
        );
        services.images().put(img, 7, pixels.clone());
        assert!(services.images().get(img, 7).is_some());
        assert!(services.images().get(img, 8).is_none());
        let e = services.render().err().expect("no GPU");
        assert_eq!(e.into_frap().code, frap_op::errors::GPU_UNAVAILABLE);
    }
    // The host is still usable after using the services.
    cx.object_mut(ob).unwrap().location = [1.0, 2.0, 3.0];
    cx.services().images().evict(img.key());
    assert!(cx.services().images().get(img, 7).is_none());
    drop(cx);
    assert_eq!(s.render_calls, 1);
    assert_eq!(
        h.state.main.objects.get(ob).unwrap().location,
        [1.0, 2.0, 3.0]
    );
}

#[test]
fn accessors_reports_results_and_snapshot() {
    let mut h = TestHost::factory();
    let mut s = TestServices::default();
    let cube = h.object("Cube");
    let me: IdRef<MeshId> = h.state.main.meshes.find("Cube").unwrap();
    let mut cx = OpCtx::new(&mut h, &mut s, spec());
    assert_eq!(cx.op().idname, "dummy.add");
    assert_eq!(cx.main().objects.len(), 3);
    cx.mesh_mut(me).unwrap();
    assert!(matches!(cx.bmesh_mut(me), Err(OpError::NotImplemented(_))));
    let snap = cx.snapshot_at(None).unwrap();
    cx.object_mut(cube).unwrap().location = [5.0, 0.0, 0.0];
    // The snapshot is a clone: later writes do not reach it.
    assert_eq!(
        snap.host.main().objects.get(cube).unwrap().location,
        [0.0; 3]
    );
    cx.report(ReportType::Warning, "careful");
    cx.result("object", cube);
    let out = cx.finish();
    assert_eq!(out.reports.len(), 1);
    assert_eq!(out.reports[0].message, "careful");
    assert_eq!(out.result["object"], Value::Id(Some(cube.any())));
    assert!(h.log.modified.contains(&cube.any()));
    assert!(h.log.modified.contains(&me.any()));
}
