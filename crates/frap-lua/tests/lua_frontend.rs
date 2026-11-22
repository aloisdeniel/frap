// SPDX-License-Identifier: GPL-3.0-or-later
//! WP 8.10 acceptance (§22.10): the Lua front-end over `Session::{invoke, get, set, call}`.
//! Run with `cargo test -p frap-lua --features lua` (add `gpu` to render the transcript).
#![cfg(feature = "lua")]

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use frap_lua::{LuaConfig, LuaFrontend};
use frap_session::{Session, SessionOptions};
use serde_json::Value as J;

fn tmpdir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("frap-lua-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn opts(cwd: &Path) -> SessionOptions {
    let mut o = SessionOptions::default();
    o.cwd = cwd.to_path_buf();
    o
}

fn session(cwd: &Path) -> Session {
    Session::new(opts(cwd)).unwrap()
}

fn front() -> LuaFrontend {
    LuaFrontend::new(session(&std::env::temp_dir())).unwrap()
}

#[track_caller]
fn run(f: &mut LuaFrontend, src: &str) -> Vec<String> {
    if let Err(e) = f.exec(src) {
        panic!(
            "{src}\n→ {}: {} ({}) {:?}",
            e.exception.as_str(),
            e.message,
            e.code.0,
            e.detail
        );
    }
    f.take_output()
}

#[test]
fn ops_data_and_context_map_onto_the_session() {
    let mut f = front();
    let out = run(
        &mut f,
        r#"
        local status = bpy.ops.mesh.primitive_cube_add{size=2, location={1, 2, 3}}
        print(status)
        local ob = bpy.context.object
        print(ob.name, ob.type, tostring(ob))
        print(ob.location.x, ob.location[1], ob.location.z)
        ob.location.x = 5
        print(ob.location.x)
        ob.location = {0, 0, 0.5}
        print(table.concat(frap.value(ob.location), ","))
        print(#bpy.data.objects, #ob.data.vertices)
        -- dot and colon calls of RNA functions
        print(ob.select_get(), ob:select_get())
        ob:select_set(false)
        print(ob.select_get())
        local e = bpy.data.objects.new{name='Empty', object_data=None}
        print(tostring(e), e.data == nil)
        bpy.context.collection.objects:link(e)
        print(bpy.data.objects['Empty'].users_collection[1].name)
        -- enum flags as sets
        -- (bool arrays are array proxies; frap.value gives the plain sequence)
        local msm = frap.value(bpy.context.scene.tool_settings.mesh_select_mode)
        print(tostring(bpy.context.scene.tool_settings.snap_elements),
            tostring(msm[1]) .. "," .. tostring(msm[2]) .. "," .. tostring(msm[3]))
        print(tostring(Set{'VERT', 'FACE'}), Set{'VERT'}.VERT, Set{'VERT'}.EDGE)
        -- operator metadata
        print(bpy.ops.object.delete.poll(), bpy.ops.mesh.bevel.poll())
        print(bpy.ops.mesh.primitive_cube_add.idname(), bpy.ops.mesh.primitive_cube_add.idname_py())
        -- iteration (Blender indices)
        local names = {}
        for i, o in pairs(bpy.data.objects) do names[#names + 1] = i .. '=' .. o.name end
        print(table.concat(names, ' '))
        -- the raw Session API
        frap.set("bpy.data.objects['Cube'].scale", {2, 2, 2})
        print(frap.get("bpy.data.objects['Cube'].scale").y)
        print(frap.call("bpy.data.objects.find", 'Cube'))
        print((frap.invoke('object.select_all', {action='SELECT'})))
        "#,
    );
    let expect = [
        "FINISHED",
        "Cube.001\tMESH\tbpy.data.objects['Cube.001']",
        "1\t2\t3",
        "5",
        "0,0,0.5",
        "4\t8",
        "true\ttrue",
        "false",
        "bpy.data.objects['Empty']\ttrue",
        "Collection",
        "{\"INCREMENT\"}\ttrue,false,false",
        "{\"FACE\", \"VERT\"}\ttrue\tfalse",
        "true\tfalse",
        "MESH_OT_primitive_cube_add\tmesh.primitive_cube_add",
        "0=Camera 1=Cube 2=Cube.001 3=Empty 4=Light",
        "2",
        "1",
        "FINISHED",
    ];
    assert_eq!(out, expect);

    // The session saw the same calls: operators, assignments and function calls are logged.
    let rs = f.take_responses();
    let kinds: Vec<_> = rs.iter().map(|r| r.kind).collect();
    assert!(kinds.contains(&"op") && kinds.contains(&"set") && kinds.contains(&"call"));
    assert!(rs.iter().all(|r| r.ok()));
    let s = f.into_session();
    assert!(s.state_dump().contains("Empty"));
}

#[test]
fn globals_persist_between_chunks_and_responses_are_ordered() {
    let mut f = front();
    run(&mut f, "cube = bpy.data.objects['Cube']");
    let out = run(&mut f, "cube.location = {1, 1, 1}; print(cube.location.y)");
    assert_eq!(out, ["1"]);
    let rs = f.take_responses();
    assert_eq!(rs.len(), 1);
    assert_eq!(rs[0].kind, "set");
    assert_eq!(
        rs[0].path.as_deref(),
        Some("bpy.data.objects['Cube'].location")
    );
}

/// Python statement → Lua statement raising the same error.
const SAME_ERRORS: &[(&str, &str)] = &[
    ("bpy.ops.mesh.foo()", "bpy.ops.mesh.foo()"),
    (
        "bpy.ops.mesh.knife_project()",
        "bpy.ops.mesh.knife_project()",
    ),
    (
        "bpy.ops.mesh.primitive_cube_add(sizee=2)",
        "bpy.ops.mesh.primitive_cube_add{sizee=2}",
    ),
    (
        "bpy.ops.mesh.primitive_cube_add(size='x')",
        "bpy.ops.mesh.primitive_cube_add{size='x'}",
    ),
    (
        "bpy.ops.mesh.primitive_cube_add(align='FOO')",
        "bpy.ops.mesh.primitive_cube_add{align='FOO'}",
    ),
    (
        "bpy.ops.mesh.primitive_cube_add(location=(1, 2))",
        "bpy.ops.mesh.primitive_cube_add{location={1, 2}}",
    ),
    (
        "bpy.ops.object.mode_set(mode='POSE')",
        "bpy.ops.object.mode_set{mode='POSE'}",
    ),
    (
        "bpy.data.objects['Cube'].foo",
        "local _ = bpy.data.objects['Cube'].foo",
    ),
    (
        "bpy.data.objects['Cube'].type = 'EMPTY'",
        "bpy.data.objects['Cube'].type = 'EMPTY'",
    ),
    ("bpy.context.object = None", "bpy.context.object = None"),
    (
        "bpy.data.objects['Nope']",
        "local _ = bpy.data.objects['Nope']",
    ),
    ("bpy.data.objects[9]", "local _ = bpy.data.objects[9]"),
    (
        "bpy.data.objects['Cube'].name = 1",
        "bpy.data.objects['Cube'].name = 1",
    ),
    (
        "bpy.data.objects['Cube'].rotation_mode = 'FOO'",
        "bpy.data.objects['Cube'].rotation_mode = 'FOO'",
    ),
    (
        "bpy.data.objects['Cube'].location = (1, 2)",
        "bpy.data.objects['Cube'].location = {1, 2}",
    ),
    (
        "bpy.data.objects['Cube'].select_set()",
        "bpy.data.objects['Cube']:select_set()",
    ),
    (
        "bpy.data.objects.remove(bpy.data.objects['Cube'], do_unlink=False)",
        "bpy.data.objects:remove(bpy.data.objects['Cube'], {do_unlink=false})",
    ),
    (
        "bpy.ops.import_scene.gltf(filepath='missing.glb')",
        "bpy.ops.import_scene.gltf{filepath='missing.glb'}",
    ),
    (
        "bpy.ops.wm.open_mainfile(filepath='missing.frap')",
        "bpy.ops.wm.open_mainfile{filepath='missing.frap'}",
    ),
];

#[test]
fn errors_carry_the_sessions_exception_code_and_message() {
    for (py, lua) in SAME_ERRORS {
        let dir = tmpdir("errors");
        let mut s = session(&dir);
        let r = s.exec_source(py).pop().unwrap();
        let want = r.error.unwrap_or_else(|| panic!("{py} did not fail"));
        let mut f = LuaFrontend::new(session(&dir)).unwrap();
        let got = f.exec(lua).expect_err(lua);
        assert_eq!(
            (got.exception, got.code, got.message.as_str()),
            (want.exception, want.code, want.message.as_str()),
            "{lua}"
        );
        assert_eq!(
            got.detail.get("lua_line"),
            Some(&J::from(1)),
            "{lua}: {:?}",
            got.detail
        );
        // The same error as a Lua value: pcall gets the table.
        let quoted = lua.replace('\\', "\\\\").replace('"', "\\\"");
        let out = run(
            &mut f,
            &format!(
                r#"local ok, e = pcall(load("{quoted}"))
                print(ok, e.exception, e.code, e.message == {msg:?}, tostring(e) == e.exception .. ': ' .. e.message, frap.is_error(e))"#,
                msg = want.message,
            ),
        );
        assert_eq!(
            out,
            [format!(
                "false\t{}\t{}\ttrue\ttrue\ttrue",
                want.exception.as_str(),
                want.code.0
            )],
            "{lua}"
        );
    }
}

#[test]
fn a_failed_call_leaves_the_scene_unchanged_and_stops_the_chunk() {
    let mut f = front();
    let before = f.with_session(Session::state_dump);
    let e = f
        .exec("bpy.data.objects['Cube'].location = {1, 2}\nbpy.data.objects['Cube'].location = {9, 9, 9}")
        .unwrap_err();
    assert_eq!(e.code.0, "RNA_LENGTH");
    assert_eq!(e.detail.get("lua_line"), Some(&J::from(1)));
    assert_eq!(f.with_session(Session::state_dump), before);
}

#[test]
fn lua_errors_and_bad_values_are_frap_errors() {
    let mut f = front();
    let e = f.exec("local x = nil\nx.y = 1").unwrap_err();
    assert_eq!(
        (e.exception.as_str(), e.code.0),
        ("RuntimeError", "LUA_ERROR")
    );
    assert!(e.message.starts_with("lua:2:"), "{}", e.message);
    assert_eq!(e.detail.get("lua_line"), Some(&J::from(2)));

    let e = f.exec("bpy.ops.mesh.primitive_cube_add{").unwrap_err();
    assert_eq!((e.exception.as_str(), e.code.0), ("SyntaxError", "SYNTAX"));

    let e = f
        .exec("bpy.data.objects['Cube'].location = {1, 2, x=3}")
        .unwrap_err();
    assert_eq!((e.exception.as_str(), e.code.0), ("TypeError", "LUA_VALUE"));

    let e = f.exec("bpy.data.objects['Cube'].name = print").unwrap_err();
    assert_eq!((e.exception.as_str(), e.code.0), ("TypeError", "LUA_VALUE"));

    let e = f.exec("bpy.ops.mesh.primitive_cube_add(2)").unwrap_err();
    assert_eq!(
        (e.exception.as_str(), e.code.0),
        ("TypeError", "OP_ARG_POSITIONAL")
    );

    let e = f.exec("error({message='mine'})").unwrap_err();
    assert_eq!((e.code.0, e.message.as_str()), ("LUA_ERROR", "mine"));

    let e = f
        .exec("local t = {}; t[1] = t; bpy.data.objects['Cube'].location = t")
        .unwrap_err();
    assert_eq!(e.code.0, "LUA_VALUE");
    assert!(e.message.contains("nested"), "{}", e.message);
}

#[test]
fn the_sandbox_has_no_os_io_or_module_loading() {
    let mut f = front();
    let out = run(
        &mut f,
        r#"
        print(os, io, package, require, debug, dofile, loadfile, collectgarbage, string.dump)
        print(load(string.char(27) .. "Lua"))
        print(load("return 1 + 1")())
        print(pcall(require or error, "os"))
        print(type(getmetatable(bpy.data)), getmetatable(bpy.data))
        print(rawget(bpy.data, "objects"))
        "#,
    );
    assert_eq!(out[0], ["nil"; 9].join("\t"));
    assert!(
        out[1].starts_with("nil\t") && out[1].contains("binary chunk"),
        "{}",
        out[1]
    );
    assert_eq!(out[2], "2");
    assert!(out[3].starts_with("false\t"), "{}", out[3]);
    assert_eq!(out[4], "string\tfrap.proxy");
    assert_eq!(out[5], "nil");
    // The proxies' metatables are locked.
    let e = f.exec("setmetatable(bpy.data, {})").unwrap_err();
    assert!(e.message.contains("protected metatable"), "{}", e.message);
}

fn timeout_front(ms: u64) -> LuaFrontend {
    let mut o = opts(&std::env::temp_dir());
    o.limits.stmt_timeout = Some(Duration::from_millis(ms));
    LuaFrontend::new(Session::new(o).unwrap()).unwrap()
}

#[test]
fn stmt_timeout_stops_a_chunk() {
    for src in [
        "while true do end",
        // pcall must not swallow the deadline
        "while true do pcall(function() while true do end end) end",
        "while true do local ok = pcall(error, 'x') end",
        "local co = coroutine.wrap(function() while true do end end); co()",
        "while true do coroutine.resume(coroutine.create(function() while true do end end)) end",
        // session calls in the loop
        "while true do local _ = bpy.data.objects['Cube'].location.x end",
    ] {
        let mut f = timeout_front(200);
        let t = Instant::now();
        let e = f.exec(src).expect_err(src);
        let took = t.elapsed();
        assert_eq!(
            (e.exception.as_str(), e.code.0),
            ("RuntimeError", "TIMEOUT"),
            "{src}: {}",
            e.message
        );
        assert_eq!(e.message, "Error: frap: limit exceeded (stmt_timeout)");
        assert_eq!(e.detail.get("stmt_timeout_ms"), Some(&J::from(200)));
        assert!(took < Duration::from_secs(5), "{src}: {took:?}");
        // The front-end is usable afterwards; the deadline is per chunk.
        assert_eq!(run(&mut f, "print(1 + 1)"), ["2"]);
    }
}

#[test]
fn the_lua_heap_is_limited() {
    let mut c = LuaConfig::default();
    c.memory_limit = 16 << 20;
    let mut f = LuaFrontend::with_config(session(&std::env::temp_dir()), c).unwrap();
    let e = f
        .exec("local t = {} for i = 1, 1e9 do t[i] = i end")
        .unwrap_err();
    assert_eq!(
        (e.exception.as_str(), e.code.0),
        ("RuntimeError", "LIMIT_EXCEEDED"),
        "{}",
        e.message
    );
    assert_eq!(run(&mut f, "print('still here')"), ["still here"]);
}

/// Runs the M1 exit transcript through the Python front-end, then its Lua re-expression in
/// the same directory (both write `out/part.*`), and returns both final states.
fn transcript_states(name: &str) -> (String, String, LuaFrontend) {
    let dir = tmpdir(name);
    // The transcript's sandbox has `out/` once the render made it; without an adapter the
    // exporter would find no folder (Blender's exporter does not create one either).
    std::fs::create_dir_all(dir.join("out")).unwrap();
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let py = std::fs::read_to_string(repo.join("tests/cli/m1/vertical_slice.in/vertical_slice.py"))
        .unwrap();
    let mut s = session(&dir);
    let rs = s.exec_source(&py);
    assert_eq!(rs.len(), 19);
    for r in &rs {
        let gpu_missing = r
            .error
            .as_ref()
            .is_some_and(|e| e.code.0 == "GPU_UNAVAILABLE");
        assert!(r.ok() || gpu_missing, "{}", r.to_json());
    }
    let py_state = s.state_dump();

    let lua = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/m1_transcript.lua"),
    )
    .unwrap();
    let mut f = LuaFrontend::new(session(&dir)).unwrap();
    run(&mut f, &lua);
    let lua_state = f.with_session(Session::state_dump);
    (py_state, lua_state, f)
}

#[test]
fn the_m1_exit_transcript_in_lua_reaches_the_same_state() {
    let (py_state, lua_state, mut f) = transcript_states("transcript-state");
    let (py_state, lua_state) = (normalize_ids(&py_state), normalize_ids(&lua_state));
    assert!(
        py_state == lua_state,
        "states differ:\n{}",
        first_diff(&py_state, &lua_state)
    );
    let out = run(
        &mut f,
        "print(imported_metallic, table.concat(cylinder_location, ','))",
    );
    assert_eq!(out, ["1\t0,0,1"]);
    // One response per session call of the transcript (16 operators and assignments, the
    // render, the append), all FINISHED but a render without an adapter.
    let rs = f.take_responses();
    assert!(rs.iter().all(|r| {
        r.ok()
            || r.error
                .as_ref()
                .is_some_and(|e| e.code.0 == "GPU_UNAVAILABLE")
    }));
}

#[test]
fn the_m1_exit_transcript_in_lua_matches_blender() {
    let (_, _, mut f) = transcript_states("transcript-blender");
    let dump = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/dump_state.lua"),
    )
    .unwrap();
    let got = run(&mut f, &dump);
    let want = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/m1_transcript_blender.txt"),
    )
    .unwrap();
    let rendered = f.with_session(|s| s.render_service_started());
    compare_facts(&got, &want, !rendered);
}

/// Compares fact lines token by token: numbers within 1e-6 (Blender prints f32 values
/// widened to f64, frap their shortest f32 form), everything else exactly.
fn compare_facts(got: &[String], want: &str, skip_render: bool) {
    let want: Vec<&str> = want.lines().collect();
    assert_eq!(
        got.len(),
        want.len(),
        "got:\n{}\nwant:\n{}",
        got.join("\n"),
        want.join("\n")
    );
    for (g, w) in got.iter().zip(&want) {
        if skip_render && w.starts_with("render png") {
            continue;
        }
        let gt: Vec<&str> = g.split(' ').collect();
        let wt: Vec<&str> = w.split(' ').collect();
        assert_eq!(gt.len(), wt.len(), "{g} / {w}");
        for (a, b) in gt.iter().zip(&wt) {
            match (a.parse::<f64>(), b.parse::<f64>()) {
                (Ok(x), Ok(y)) => assert!((x - y).abs() <= 1e-6, "{g} / {w}"),
                _ => assert_eq!(a, b, "{g} / {w}"),
            }
        }
    }
}

/// `session_uid`s, `IdKey`s and ID `generation`s come from process-wide counters, so two
/// sessions of one process never share them: keys and uids are renumbered in order of first
/// appearance (which keeps every reference between IDs), generations dropped.
fn normalize_ids(dump: &str) -> String {
    let mut seen: std::collections::HashMap<String, usize> = Default::default();
    let mut out = String::with_capacity(dump.len());
    let mut rest = dump;
    let prefixes = ["IdKey(", "session_uid: ", "generation: ", ">("];
    loop {
        let next = prefixes
            .iter()
            .filter_map(|p| rest.find(p).map(|i| (i, *p)))
            .min_by_key(|(i, _)| *i);
        let Some((i, p)) = next else {
            out.push_str(rest);
            return out;
        };
        out.push_str(&rest[..i + p.len()]);
        rest = &rest[i + p.len()..];
        let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
        let n = &rest[..digits];
        if digits == 0 {
            continue;
        }
        if p == "generation: " {
            out.push('#');
        } else {
            let k = seen.len();
            let id = *seen.entry(n.to_owned()).or_insert(k);
            out.push_str(&format!("#{id}"));
        }
        rest = &rest[digits..];
    }
}

fn first_diff(a: &str, b: &str) -> String {
    let i = a
        .bytes()
        .zip(b.bytes())
        .position(|(x, y)| x != y)
        .unwrap_or(a.len().min(b.len()));
    let lo = i.saturating_sub(200);
    format!(
        "python …{}…\nlua    …{}…",
        &a[lo..(i + 200).min(a.len())],
        &b[lo..(i + 200).min(b.len())]
    )
}

fn locate_blender() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("BLENDER_BIN") {
        return Some(PathBuf::from(p));
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    [
        home.join("Applications/Blender-5.2.app/Contents/MacOS/Blender"),
        home.join("Applications/Blender.app/Contents/MacOS/Blender"),
        PathBuf::from("/Applications/Blender.app/Contents/MacOS/Blender"),
        PathBuf::from("/usr/bin/blender"),
    ]
    .into_iter()
    .find(|p| p.is_file())
}

/// The committed Blender facts are what Blender 5.2 prints today (skipped without Blender;
/// `FRAP_REQUIRE_BLENDER=1` makes that a failure).
#[test]
fn the_blender_reference_is_current() {
    let Some(blender) = locate_blender() else {
        assert!(
            std::env::var_os("FRAP_REQUIRE_BLENDER").is_none(),
            "Blender not found"
        );
        eprintln!("skipped: Blender 5.2 not found (set BLENDER_BIN)");
        return;
    };
    let dir = tmpdir("blender");
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/blender/m1_transcript_state.py");
    let out_file = dir.join("state.txt");
    let st = std::process::Command::new(blender)
        .args(["-b", "--factory-startup", "--python"])
        .arg(&script)
        .arg("--")
        .arg(&out_file)
        .current_dir(&dir)
        .output()
        .unwrap();
    assert!(
        st.status.success(),
        "{}",
        String::from_utf8_lossy(&st.stderr)
    );
    let live = std::fs::read_to_string(&out_file).unwrap();
    let committed = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/m1_transcript_blender.txt"),
    )
    .unwrap();
    assert_eq!(live, committed);
}

/// Cost of the proxy walk against the statement form (RFC 0002 §5; `--ignored --nocapture`,
/// numbers depend on the profile).
#[test]
#[ignore = "measurement for RFC 0002, not a check"]
fn measure_lua_against_statements() {
    const N: usize = 2000;
    let mut s = session(&std::env::temp_dir());
    let src = "bpy.data.objects['Cube'].location[0]\n".repeat(N);
    let t = Instant::now();
    let rs = s.exec_source(&src);
    assert!(rs.iter().all(|r| r.ok()));
    let py_read = t.elapsed();

    let mut f = front();
    let t = Instant::now();
    run(
        &mut f,
        &format!("for i = 1, {N} do local _ = bpy.data.objects['Cube'].location.x end"),
    );
    let lua_read = t.elapsed();
    let t = Instant::now();
    run(
        &mut f,
        &format!(
            "local ob = bpy.data.objects['Cube'] for i = 1, {N} do local _ = ob.location.x end"
        ),
    );
    let lua_read_cached = t.elapsed();

    let src = "bpy.data.objects['Cube'].location = (1, 2, 3)\n".repeat(N);
    let t = Instant::now();
    s.exec_source(&src);
    let py_set = t.elapsed();
    let t = Instant::now();
    run(
        &mut f,
        &format!(
            "local ob = bpy.data.objects['Cube'] for i = 1, {N} do ob.location = {{1, 2, 3}} end"
        ),
    );
    let lua_set = t.elapsed();
    let per = |d: Duration| d.as_secs_f64() * 1e6 / N as f64;
    eprintln!(
        "per statement (µs): python read {:.1}, lua read {:.1} (cached object {:.1}), python set {:.1}, lua set {:.1}",
        per(py_read),
        per(lua_read),
        per(lua_read_cached),
        per(py_set),
        per(lua_set)
    );
}
