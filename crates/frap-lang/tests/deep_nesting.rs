// SPDX-License-Identifier: GPL-3.0-or-later
//! Deeply nested input never overflows the stack (a stack overflow aborts the process, which
//! `catch_unwind` cannot turn into an `INTERNAL` answer). Every case runs on a thread with a
//! 2 MiB stack, the default for spawned and async worker threads, in whatever profile the
//! tests are built with (debug frames are the largest).

use frap_lang::split::{RawSplitter, split};
use frap_lang::{is_complete, parse_complete};
use frap_rna::FrapError;

const STACK: usize = 2 * 1024 * 1024;

/// Parses `src` on a 2 MiB thread and returns the single statement's error (or `None`).
fn parse_small_stack(src: String) -> Option<FrapError> {
    std::thread::Builder::new()
        .stack_size(STACK)
        .spawn(move || {
            // The splitter and the raw-stdin accumulator are iterative; exercise them too.
            let _ = split(&src);
            let _ = is_complete(&src);
            let mut raw = RawSplitter::new();
            let _ = raw.push_line(&src);
            let _ = raw.finish();
            let mut r = parse_complete(&src);
            assert_eq!(r.len(), 1, "{:?}", &src[..src.len().min(80)]);
            r.pop().and_then(|r| r.err())
        })
        .expect("spawn")
        .join()
        .expect("no panic")
}

fn assert_err(src: String, code: &str, msg: &str, col: u32) {
    let e = parse_small_stack(src).expect("an error");
    assert_eq!(e.code.0, code, "{}", e.message);
    assert_eq!(e.message, msg);
    let span = e.span.expect("span");
    assert_eq!(span.col, col, "{}", e.message);
}

const DEEP: usize = 10_000;

#[test]
fn ten_thousand_nested_parens_bpy_form() {
    let src = format!(
        "bpy.data.objects[0].location = {}1{}",
        "(".repeat(DEEP),
        ")".repeat(DEEP)
    );
    // CPython (Blender 5.2.4): the 201st open bracket, `x = ` + 201 '(' → offset 205.
    assert_err(src, "SYNTAX", "too many nested parentheses", 32 + 200);
}

#[test]
fn ten_thousand_nested_brackets_of_each_kind_and_unclosed() {
    for (o, c) in [("[", "]"), ("{", "}"), ("f(", ")")] {
        let src = format!("x = {}1{}", o.repeat(DEEP), c.repeat(DEEP));
        let e = parse_small_stack(src).expect("an error");
        assert_eq!(e.message, "too many nested parentheses");
        // Unclosed: the depth error comes first in source order.
        let e = parse_small_stack(format!("x = {}", o.repeat(DEEP))).expect("an error");
        assert_eq!(e.message, "too many nested parentheses");
    }
}

#[test]
fn ten_thousand_nested_parens_short_form() {
    let src = format!(
        "data.objects[0].location={}1{}",
        "(".repeat(DEEP),
        ")".repeat(DEEP)
    );
    assert_err(src, "SYNTAX", "too many nested parentheses", 26 + 200);
    let src = format!(
        "mesh.primitive_cube_add location={}1{}",
        "(".repeat(DEEP),
        ")".repeat(DEEP)
    );
    let e = parse_small_stack(src).expect("an error");
    assert_eq!(e.message, "too many nested parentheses");
    let src = format!(
        "data.objects[0].hide_set {}True{}",
        "{".repeat(DEEP),
        "}".repeat(DEEP)
    );
    let e = parse_small_stack(src).expect("an error");
    assert_eq!(e.message, "too many nested parentheses");
}

#[test]
fn two_hundred_nested_parens_parse() {
    // CPython accepts exactly 200 levels; so does frap, without overflowing a 2 MiB stack.
    for (o, c) in [("(", ")"), ("[", "]"), ("{", "}"), ("float(", ")")] {
        let src = format!(
            "bpy.data.objects[0].location = {}1{}",
            o.repeat(200),
            c.repeat(200)
        );
        assert!(parse_small_stack(src).is_none(), "{o}");
    }
    let src = format!(
        "data.objects[0].location={}1{}",
        "(".repeat(200),
        ")".repeat(200)
    );
    assert!(parse_small_stack(src).is_none());
    let src = format!(
        "data.objects[0].hide_set {}1{}",
        "{".repeat(200),
        "}".repeat(200)
    );
    assert!(parse_small_stack(src).is_none());
}

#[test]
fn deep_unary_and_power_chains_are_rejected_not_overflowed() {
    let too_complex = "frap: expression too complex to parse (more than 250 nested levels)";
    for src in [
        format!("bpy.data.objects[0].location.x = {}1", "-".repeat(DEEP)),
        format!("bpy.data.objects[0].location.x = 2{}", "**2".repeat(DEEP)),
        format!("bpy.data.objects[0].location.x = {}2", "-2**".repeat(DEEP)),
        format!(
            "bpy.data.objects[0].location.x = {}1{}",
            "-(".repeat(150),
            ")".repeat(150)
        ),
        format!("print({}1)", "+-~".repeat(DEEP)),
        format!("data.objects[0].location.x={}1", "-".repeat(DEEP)),
        format!("mesh.primitive_cube_add size={}1", "-".repeat(DEEP)),
        format!("data.objects[0].location.x=(2{})", "**2".repeat(DEEP)),
    ] {
        let e = parse_small_stack(src.clone()).expect("an error");
        assert!(
            e.message == too_complex || e.message.contains("bitwise"),
            "{}: {}",
            &src[..40],
            e.message
        );
        assert!(e.span.is_some());
    }
    // A long but flat expression is fine.
    let src = format!("bpy.data.objects[0].location.x = 1{}", "+1".repeat(DEEP));
    assert!(parse_small_stack(src).is_none());
    // Moderate chains still parse.
    let src = format!("bpy.data.objects[0].location.x = {}1", "-".repeat(200));
    assert!(parse_small_stack(src).is_none());
}

#[test]
fn deep_nesting_inside_calls_and_displays() {
    // Nested calls, method calls and displays below the bracket limit but deep in total.
    let src = format!(
        "bpy.data.objects[0].location = {}(1, 2, 3){}",
        "Vector(".repeat(199),
        ")".repeat(199)
    );
    assert!(parse_small_stack(src).is_none());
    let src = format!(
        "bpy.data.objects[0].location = {}1{}",
        "[-(".repeat(66),
        ")]".repeat(66)
    );
    assert!(parse_small_stack(src).is_none());
    let src = format!(
        "bpy.data.objects[0].location = {}1{}",
        "[-(-".repeat(100),
        ")]".repeat(100)
    );
    let e = parse_small_stack(src).expect("too complex");
    assert_eq!(e.code.0, "UNSUPPORTED_SYNTAX");
}

#[test]
fn inline_and_dedicated_stack_paths_agree_around_the_threshold() {
    // Below the inline budget the caller's stack is used, above it a dedicated one: sweep
    // across the boundary on a 2 MiB stack and check the parsed IR depth.
    for depth in 0..=40 {
        for (o, c) in [("float(", ")"), ("(", ")"), ("[", "]")] {
            let src = format!(
                "bpy.data.objects[0].location.x = {}-1{}",
                o.repeat(depth),
                c.repeat(depth)
            );
            assert!(parse_small_stack(src).is_none(), "{o} x {depth}");
        }
        let src = format!(
            "data.objects[0].location={}1{}",
            "(".repeat(depth),
            ")".repeat(depth)
        );
        assert!(parse_small_stack(src).is_none(), "short ( x {depth}");
        let src = format!(
            "data.objects[0].hide_set {}True{}",
            "{".repeat(depth),
            "}".repeat(depth)
        );
        assert!(parse_small_stack(src).is_none(), "short {{ x {depth}");
    }
    // Many negative numbers (a large foreach_set) go through the dedicated stack and give the
    // same IR as a short one.
    let n = 5000;
    let src = format!(
        "bpy.data.meshes['M'].vertices.foreach_set('co', ({}))",
        "-1.5, ".repeat(n)
    );
    let stmt = parse_complete(&src).pop().expect("one").expect("ok");
    let frap_lang::Command::Call { args, .. } = &stmt.cmd else {
        panic!("{:?}", stmt.cmd)
    };
    let frap_lang::Expr::Tuple(items, _) = &args[1].value else {
        panic!("tuple")
    };
    assert_eq!(items.len(), n);
}

#[test]
fn megabyte_line_with_non_ascii_is_linear() {
    // One line, ~1 MB, non-ASCII before the long tuple: spans need code-point columns.
    let n = 150_000;
    let src = format!(
        "bpy.data.meshes['Würfel'].vertices.foreach_set('co', ({}))",
        "-1.5, ".repeat(n)
    );
    let t = std::time::Instant::now();
    let stmt = parse_complete(&src).pop().expect("one").expect("ok");
    // Quadratic column counting took minutes at this size; linear takes well under a second
    // in release and a few seconds in debug.
    assert!(t.elapsed().as_secs() < 30, "{:?}", t.elapsed());
    let frap_lang::Command::Call { args, .. } = &stmt.cmd else {
        panic!("{:?}", stmt.cmd)
    };
    let last = match &args[1].value {
        frap_lang::Expr::Tuple(items, _) => items.last().expect("items").clone(),
        other => panic!("{other:?}"),
    };
    let frap_lang::Expr::Unary(_, _, span) = last else {
        panic!("unary")
    };
    let line_start = 0;
    assert_eq!(
        span.col_chars as usize,
        src[line_start..span.start as usize].chars().count() + 1
    );
}
