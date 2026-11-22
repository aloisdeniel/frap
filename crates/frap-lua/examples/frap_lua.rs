// SPDX-License-Identifier: GPL-3.0-or-later
//! What a `frap --lua` front-end would look like (RFC 0002 §7): runs a Lua file (or stdin)
//! against a factory session in the current directory and writes one JSON line per session
//! call, then the `print` output, then a done record.
//!
//! ```text
//! cargo run -p frap-lua --features lua --example frap_lua -- script.lua
//! ```

use std::io::Read;

use frap_lua::LuaFrontend;
use frap_session::{Session, SessionOptions};

fn main() {
    let src = match std::env::args().nth(1) {
        Some(path) => std::fs::read_to_string(&path).unwrap_or_else(|e| {
            eprintln!("frap_lua: {path}: {e}");
            std::process::exit(2);
        }),
        None => {
            let mut s = String::new();
            if let Err(e) = std::io::stdin().read_to_string(&mut s) {
                eprintln!("frap_lua: stdin: {e}");
                std::process::exit(2);
            }
            s
        }
    };
    let session = Session::new(SessionOptions::default()).unwrap_or_else(|e| {
        eprintln!("frap_lua: {}", e.message);
        std::process::exit(2);
    });
    let mut front = LuaFrontend::new(session).unwrap_or_else(|e| {
        eprintln!("frap_lua: {}", e.message);
        std::process::exit(2);
    });
    let result = front.exec(&src);
    for r in front.take_responses() {
        println!("{}", r.to_json());
    }
    for line in front.take_output() {
        println!("{}", serde_json::json!({ "event": "print", "text": line }));
    }
    let error = result.as_ref().err().map(|e| {
        serde_json::json!({
            "exception": e.exception.as_str(),
            "code": e.code.0,
            "message": e.message,
            "detail": e.detail,
            "hint": e.hint,
        })
    });
    println!(
        "{}",
        serde_json::json!({ "event": "done", "ok": result.is_ok(), "error": error })
    );
    std::process::exit(i32::from(result.is_err()));
}
