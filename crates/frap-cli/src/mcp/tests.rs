// SPDX-License-Identifier: GPL-3.0-or-later
//! In-process tests of the MCP server (the binary-level transcript is in
//! `crates/frap-cli/tests/mcp.rs` and `tests/cli/m1/cli/mcp_*.toml`).

use super::*;
use frap_session::SessionOptions;

fn server(opts: DescribeOptions) -> Server {
    Server::new(Session::new(SessionOptions::default()).unwrap(), opts).unwrap()
}

fn call(s: &mut Server, id: i64, method: &str, params: J) -> J {
    s.handle_message(json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}))
        .expect("a request gets a response")
}

fn text(result: &J) -> &str {
    result["result"]["content"][0]["text"].as_str().unwrap()
}

#[test]
fn handshake_negotiates_the_protocol_version() {
    let mut s = server(DescribeOptions::default());
    let r = call(
        &mut s,
        1,
        "initialize",
        json!({"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "t", "version": "0"}}),
    );
    assert_eq!(r["jsonrpc"], "2.0");
    assert_eq!(r["id"], 1);
    assert_eq!(r["result"]["protocolVersion"], "2025-06-18");
    assert_eq!(r["result"]["capabilities"]["tools"]["listChanged"], false);
    assert_eq!(r["result"]["serverInfo"]["name"], "frap");
    assert!(
        s.handle_message(json!({"jsonrpc": "2.0", "method": "notifications/initialized"}))
            .is_none()
    );
    assert!(s.initialized());

    let mut s = server(DescribeOptions::default());
    let r = call(
        &mut s,
        1,
        "initialize",
        json!({"protocolVersion": "1999-01-01"}),
    );
    assert_eq!(r["result"]["protocolVersion"], PROTOCOL_VERSIONS[0]);
    assert_eq!(s.protocol_version(), Some(PROTOCOL_VERSIONS[0]));
}

#[test]
fn tools_list_is_the_describe_toolset() {
    let opts = DescribeOptions {
        tool_namespaces: vec!["mesh".into()],
        implemented_only: true,
        ..DescribeOptions::default()
    };
    let mut s = server(opts.clone());
    let r = call(&mut s, 2, "tools/list", json!({}));
    let anthropic = frap_schema::tools::anthropic(&opts).unwrap();
    let listed = r["result"]["tools"].as_array().unwrap();
    let expected = anthropic.as_array().unwrap();
    assert_eq!(listed.len(), expected.len());
    assert_eq!(listed.len(), 3);
    for (l, a) in listed.iter().zip(expected) {
        assert_eq!(l["name"], a["name"]);
        assert_eq!(l["description"], a["description"]);
        assert_eq!(l["inputSchema"], a["input_schema"]);
    }
}

#[test]
fn tools_call_keeps_state_across_calls() {
    let mut s = server(DescribeOptions::default());
    let r = call(
        &mut s,
        3,
        "tools/call",
        json!({"name": "frap_exec", "arguments": {"statements": "bpy.data.objects['Cube'].location = (1, 2, 3)"}}),
    );
    assert_eq!(r["result"]["isError"], false);
    let r = call(
        &mut s,
        4,
        "tools/call",
        json!({"name": "frap_exec", "arguments": {"statements": "bpy.data.objects['Cube'].location", "on_error": null}}),
    );
    let lines: Vec<J> = text(&r)
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0]["id"], 4);
    assert_eq!(lines[0]["seq"], 2);
    assert_eq!(lines[0]["value"], json!([1, 2, 3]));
    assert_eq!(
        lines[1],
        json!({"id": 4, "event": "done", "ok": true,
               "counts": {"finished": 1, "error": 0, "cancelled": 0, "skipped": 0}})
    );
}

#[test]
fn exec_errors_and_on_error() {
    let mut s = server(DescribeOptions::default());
    let r = call(
        &mut s,
        5,
        "tools/call",
        json!({"name": "frap_exec", "arguments": {"statements": "bpy.ops.mesh.nope()\nprint(1)"}}),
    );
    assert_eq!(r["result"]["isError"], true);
    let lines: Vec<J> = text(&r)
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines[0]["error"]["code"], "OP_UNKNOWN");
    assert_eq!(lines[1]["status"], "SKIPPED");
    assert_eq!(lines[2]["counts"]["skipped"], 1);

    let r = call(
        &mut s,
        6,
        "tools/call",
        json!({"name": "frap_exec", "arguments": {"statements": "bpy.ops.mesh.nope()\nprint(1)", "on_error": "continue"}}),
    );
    assert!(text(&r).contains("\"status\":\"FINISHED\""));

    for bad in [
        json!({}),
        json!({"statements": 3}),
        json!({"statements": "", "on_error": "stop"}),
        json!({"statements": "", "extra": 1}),
    ] {
        let r = call(
            &mut s,
            7,
            "tools/call",
            json!({"name": "frap_exec", "arguments": bad}),
        );
        assert_eq!(r["result"]["isError"], true, "{bad}");
        assert!(text(&r).starts_with("frap_exec: "));
    }
}

#[test]
fn describe_tool() {
    let mut s = server(DescribeOptions::default());
    let r = call(
        &mut s,
        8,
        "tools/call",
        json!({"name": "frap_describe", "arguments": {"target": "mesh.primitive_cube_add"}}),
    );
    assert_eq!(r["result"]["isError"], false);
    let v: J = serde_json::from_str(text(&r)).unwrap();
    assert_eq!(v["idname"], "mesh.primitive_cube_add");
    let r = call(
        &mut s,
        9,
        "tools/call",
        json!({"name": "frap_describe", "arguments": {"target": "op:mesh.primitive_cube_ad"}}),
    );
    assert_eq!(r["result"]["isError"], true);
    let v: J = serde_json::from_str(text(&r)).unwrap();
    assert_eq!(v["error"]["code"], "DESCRIBE_TARGET");
    assert_eq!(v["error"]["exception"], "KeyError");
    assert_eq!(
        v["error"]["detail"]["did_you_mean"][0],
        "mesh.primitive_cube_add"
    );
}

#[test]
fn namespace_tool_runs_the_operator() {
    let mut s = server(DescribeOptions {
        tool_namespaces: vec!["mesh".into()],
        ..DescribeOptions::default()
    });
    let r = call(
        &mut s,
        10,
        "tools/call",
        json!({"name": "frap_ops_mesh", "arguments": {"call": {"op": "primitive_cube_add", "args": {"size": 4}}}}),
    );
    assert_eq!(r["result"]["isError"], false, "{r}");
    let r = call(
        &mut s,
        11,
        "tools/call",
        json!({"name": "frap_exec", "arguments": {"statements": "bpy.data.meshes['Cube.001'].vertices[0].co"}}),
    );
    let v: J = serde_json::from_str(text(&r).lines().next().unwrap()).unwrap();
    let co: Vec<f64> = v["value"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_f64().unwrap().abs())
        .collect();
    assert_eq!(co, vec![2.0, 2.0, 2.0], "size=4 reached the operator: {v}");
    // Not enabled: unknown tool.
    let r = call(
        &mut s,
        12,
        "tools/call",
        json!({"name": "frap_ops_object", "arguments": {}}),
    );
    assert_eq!(r["error"]["code"], INVALID_PARAMS);
    let r = call(
        &mut s,
        13,
        "tools/call",
        json!({"name": "frap_ops_mesh", "arguments": {}}),
    );
    assert_eq!(r["result"]["isError"], true);
}

#[test]
fn json_rpc_errors() {
    let mut s = server(DescribeOptions::default());
    let r = s.handle_line(b"{not json").unwrap();
    assert_eq!(r["error"]["code"], PARSE_ERROR);
    assert_eq!(r["id"], J::Null);
    let r = call(&mut s, 1, "resources/list", json!({}));
    assert_eq!(r["error"]["code"], METHOD_NOT_FOUND);
    let r = s
        .handle_message(json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": [1]}))
        .unwrap();
    assert_eq!(r["error"]["code"], INVALID_PARAMS);
    let r = s
        .handle_message(json!({"jsonrpc": "1.0", "id": 3, "method": "ping"}))
        .unwrap();
    assert_eq!(r["error"]["code"], INVALID_REQUEST);
    let r = s
        .handle_message(json!({"jsonrpc": "2.0", "id": null, "method": "ping"}))
        .unwrap();
    assert_eq!(r["error"]["code"], INVALID_REQUEST);
    let r = s.handle_message(json!([1])).unwrap();
    assert_eq!(r["error"]["code"], INVALID_REQUEST);
    let r = s.handle_line(b"[]").unwrap();
    assert_eq!(r["error"]["code"], INVALID_REQUEST);
    let r = call(&mut s, 4, "ping", json!(null));
    assert_eq!(r["result"], json!({}));
    let r = call(&mut s, 5, "tools/call", json!({"name": "nope"}));
    assert_eq!(r["error"]["code"], INVALID_PARAMS);
    // Client responses and unknown notifications get no answer.
    assert!(
        s.handle_message(json!({"jsonrpc": "2.0", "id": 9, "result": {}}))
            .is_none()
    );
    assert!(
        s.handle_message(json!({"jsonrpc": "2.0", "method": "notifications/cancelled"}))
            .is_none()
    );
    // Batches.
    let r = s
        .handle_line(br#"[{"jsonrpc":"2.0","id":1,"method":"ping"},{"jsonrpc":"2.0","method":"notifications/initialized"}]"#)
        .unwrap();
    assert_eq!(r.as_array().unwrap().len(), 1);
}

#[test]
fn stdio_loop() {
    let mut s = server(DescribeOptions::default());
    let input = concat!(
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25"}}"#,
        "\n",
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        "\n\n",
        r#"{"jsonrpc":"2.0","id":2,"method":"ping"}"#,
        "\n",
    );
    let mut out = Vec::new();
    run(&mut s, input.as_bytes(), &mut out).unwrap();
    let lines: Vec<J> = String::from_utf8(out)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[1], json!({"jsonrpc": "2.0", "id": 2, "result": {}}));
}
