// SPDX-License-Identifier: GPL-3.0-or-later
//! CONTRACT signatures of the L11/L12 crates (ARCHITECTURE §3.4): the `Response` JSON shape and
//! serve framing (frap-session, §11.6, §13.1–§13.2) and `GoldenState` (§20.3). The L0–L10
//! contracts are pinned in `crates/frap-ops-all/tests/contracts.rs`.
#![allow(dead_code, clippy::type_complexity)]

use std::collections::BTreeMap;
use std::path::PathBuf;

use frap_golden::{GoldenState, locate_blender};
use frap_rna::{FrapError, Input, Report};
use frap_session::{
    DoneRecord, Limits, OnError, Request, Response, Session, SessionOptions, Startup, Status,
    Verbosity,
};
use indexmap::IndexMap;
use serde_json::{Map, Value as J, json};
use smol_str::SmolStr;

fn session_api(s: &mut Session) {
    let _: fn(SessionOptions) -> Result<Session, FrapError> = Session::new;
    let _: Vec<Response> = s.exec_source("bpy.data.objects['Cube'].location");
    let _: fn(&mut Session, Request) -> Vec<Response> = Session::exec_request;
    let _: fn(&mut Session, &str, IndexMap<String, Input>) -> Response = Session::invoke;
    let _: fn(&mut Session, &str) -> Response = Session::get;
    let _: fn(&mut Session, &str, Input) -> Response = Session::set;
    let _: fn(&mut Session, &str, Vec<Input>, IndexMap<String, Input>) -> Response = Session::call;
    let _: fn(J, &[Response]) -> DoneRecord = DoneRecord::from_responses;
}

fn session_options(o: &SessionOptions, s: &Session) {
    let _: (&PathBuf, &Startup, Verbosity, usize, usize, &Limits, bool) = (
        &o.cwd,
        &o.startup,
        o.verbosity,
        o.max_items,
        o.max_array,
        &o.limits,
        o.fail_fast,
    );
    let _: (u64, Option<std::time::Duration>) = (o.limits.max_elements, o.limits.stmt_timeout);
    let _ = [
        Startup::Factory,
        Startup::Empty,
        Startup::File(PathBuf::new()),
    ];
    let _ = [Verbosity::Minimal, Verbosity::Normal, Verbosity::Full];
    let _: &SessionOptions = s.options();
}

fn response_fields(r: &Response) {
    let _: (&Option<J>, u64, u32, &str, &Option<SmolStr>, Status) =
        (&r.id, r.seq, r.line, r.kind, &r.op, r.status);
    let _: (
        &Option<String>,
        &Option<String>,
        &Option<J>,
        &Option<J>,
        &Vec<Report>,
    ) = (&r.path, &r.func, &r.value, &r.rna, &r.reports);
    let _: (
        &Option<Map<String, J>>,
        &Option<J>,
        &Option<FrapError>,
        &Option<J>,
        &Option<J>,
        &Vec<J>,
    ) = (
        &r.result,
        &r.changes,
        &r.error,
        &r.context,
        &r.edit_mesh,
        &r.hints,
    );
    let _: J = r.to_json();
    let _ = [
        Status::Finished,
        Status::Cancelled,
        Status::Error,
        Status::Skipped,
    ];
}

fn golden(g: GoldenState) {
    let _ = (g.blender, g.lines, g.context);
    let _: (
        BTreeMap<String, frap_golden::GObject>,
        BTreeMap<String, frap_golden::GMesh>,
    ) = (g.objects, g.meshes);
    let _: (
        BTreeMap<String, frap_golden::GMesh>,
        BTreeMap<String, frap_golden::GMaterial>,
    ) = (g.evaluated, g.materials);
    let _: (
        BTreeMap<String, frap_golden::GAction>,
        BTreeMap<String, frap_golden::GFile>,
    ) = (g.actions, g.files);
    let _: fn() -> Option<PathBuf> = locate_blender;
}

#[test]
fn serve_framing_round_trips() {
    let req: Request =
        serde_json::from_value(json!({"id": 7, "src": "bpy.ops.mesh.primitive_cube_add()"}))
            .expect("request parses");
    assert_eq!(req.on_error, OnError::SkipRest);
    let req: Request =
        serde_json::from_value(json!({"id": "a", "src": "", "on_error": "continue"})).unwrap();
    assert_eq!(req.on_error, OnError::Continue);
    let done = DoneRecord {
        id: json!(7),
        event: "done".into(),
        ok: false,
        counts: frap_session::serve::DoneCounts {
            finished: 1,
            error: 1,
            cancelled: 0,
            skipped: 1,
        },
    };
    assert_eq!(
        serde_json::to_value(&done).unwrap(),
        json!({"id": 7, "event": "done", "ok": false,
               "counts": {"finished": 1, "error": 1, "cancelled": 0, "skipped": 1}})
    );
}

#[test]
fn golden_state_json_round_trips() {
    let g = GoldenState {
        blender: None,
        lines: vec![],
        objects: BTreeMap::new(),
        meshes: BTreeMap::new(),
        evaluated: BTreeMap::new(),
        materials: BTreeMap::new(),
        actions: BTreeMap::new(),
        context: Default::default(),
        files: BTreeMap::new(),
    };
    let j = serde_json::to_string(&g).unwrap();
    let back: GoldenState = serde_json::from_str(&j).unwrap();
    assert_eq!(back, g);
}
