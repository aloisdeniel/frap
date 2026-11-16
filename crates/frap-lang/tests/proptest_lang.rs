// SPDX-License-Identifier: GPL-3.0-or-later
//! Property tests (ARCHITECTURE §22.3 WP 1.3): the front-ends never panic, every statement
//! and error has a consistent span, and randomly generated calls written in both syntaxes
//! lower to the same IR.
//!
//! The default case count keeps `cargo test` fast; the 10-minute run of the acceptance is
//! `PROPTEST_CASES=2000000 cargo test -p frap-lang --release --test proptest_lang`
//! (and `cargo +nightly fuzz run lang_parse -- -max_total_time=600` where cargo-fuzz exists).

mod common;

use frap_lang::split::{RawSplitter, split};
use frap_lang::*;
use frap_rna::ptr::py_str_repr;
use proptest::prelude::*;

/// Fragments that steer random input into every parser path.
const FRAGMENTS: &[&str] = &[
    "bpy",
    ".",
    "ops",
    "data",
    "context",
    "D",
    "C",
    "mesh",
    "primitive_cube_add",
    "objects",
    "['Cube']",
    "[0]",
    "[1:2]",
    "(",
    ")",
    "[",
    "]",
    "{",
    "}",
    ",",
    "=",
    "+=",
    "-=",
    "**=",
    " ",
    "  ",
    "\n",
    "\n\n",
    ";",
    "'",
    "\"",
    "'''",
    "1",
    "-",
    "+",
    "*",
    "/",
    "@",
    "**",
    "//",
    "%",
    "2.5e3",
    "0x1F",
    "0o9",
    "01",
    "1_0",
    "1j",
    ".5",
    "True",
    "true",
    "none",
    "None",
    "Vector",
    "Matrix",
    ".Translation",
    "math",
    ".pi",
    "radians",
    "mathutils",
    "#",
    "# c\n",
    "\\",
    "\\\n",
    "Würfel",
    "🧊",
    "é",
    ":",
    "::",
    "print",
    "import",
    "from",
    "for",
    "if",
    "lambda",
    "key=",
    "TRANSFORM_OT_translate.value=",
    "x",
    "_",
    "€",
    "\u{a0}",
    "\t",
    "\r\n",
    "r'",
    "f'",
    "b'",
    "'a'",
    "\"b\"",
    "'\\x4'",
    "'\\u00e9'",
    "poll()",
    ".poll",
    "select_set",
    "==",
    "!=",
    "<",
    "and",
    "not",
    "in",
    "=1",
    "=0,0,1",
    "=FACE",
    "{A,B}",
    "Cube.001",
    "my-obj",
    "set()",
    "*",
];

fn soup() -> impl Strategy<Value = String> {
    prop::collection::vec(prop::sample::select(FRAGMENTS), 0..40).prop_map(|v| v.concat())
}

/// 512 cases by default; `PROPTEST_CASES` overrides (long runs).
fn cfg() -> ProptestConfig {
    let cases = std::env::var("PROPTEST_CASES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(512);
    ProptestConfig {
        cases,
        ..ProptestConfig::default()
    }
}

fn exercise(src: &str) {
    common::check_all_spans(src);
    let pieces = split(src);
    for p in &pieces {
        assert!(p.start <= p.end && p.end <= src.len());
        assert!(src.is_char_boundary(p.start) && src.is_char_boundary(p.end));
    }
    let _ = is_complete(src);
    let mut r = RawSplitter::new();
    let mut chunks = Vec::new();
    for line in src.split_inclusive('\n') {
        chunks.extend(r.push_line(line));
    }
    chunks.extend(r.finish());
    for c in chunks {
        common::check_all_spans(&c.src);
        let _ = parse_complete_at(&c.src, c.first_line);
    }
}

proptest! {
    #![proptest_config(cfg())]

    #[test]
    fn arbitrary_text_never_panics(src in any::<String>()) {
        exercise(&src);
    }

    #[test]
    fn token_soup_never_panics(src in soup()) {
        exercise(&src);
    }
}

// ---------------------------------------------------------------------- two-syntax round trip

#[derive(Clone, Debug)]
enum V {
    Int(i64),
    Float(f64),
    Str(String),
    Enum(String),
    Bool(bool, bool),
    None(bool),
    Tuple(Vec<V>),
    Set(Vec<String>),
}

fn ident() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9_]{0,7}".prop_filter("reserved word", |s| {
        !matches!(
            s.as_str(),
            "bpy"
                | "data"
                | "context"
                | "print"
                | "import"
                | "from"
                | "pass"
                | "true"
                | "false"
                | "none"
                | "for"
                | "while"
                | "if"
                | "elif"
                | "else"
                | "def"
                | "class"
                | "with"
                | "try"
                | "except"
                | "finally"
                | "del"
                | "return"
                | "raise"
                | "assert"
                | "global"
                | "nonlocal"
                | "lambda"
                | "async"
                | "await"
                | "yield"
                | "break"
                | "continue"
                | "and"
                | "or"
                | "not"
                | "in"
                | "is"
        )
    })
}

fn scalar() -> impl Strategy<Value = V> {
    prop_oneof![
        (-1_000_000i64..1_000_000).prop_map(V::Int),
        prop::num::f64::NORMAL.prop_map(V::Float),
        (-1e6f64..1e6).prop_map(V::Float),
        "[ -~éü🧊]{0,8}".prop_map(V::Str),
        "[A-Z][A-Z0-9_]{0,7}".prop_map(V::Enum),
        (any::<bool>(), any::<bool>()).prop_map(|(b, l)| V::Bool(b, l)),
        any::<bool>().prop_map(V::None),
    ]
}

fn value() -> impl Strategy<Value = V> {
    prop_oneof![
        4 => scalar(),
        1 => prop::collection::vec(scalar(), 2..5).prop_map(V::Tuple),
        1 => prop::collection::vec("[A-Z][A-Z_]{0,5}", 1..4).prop_map(V::Set),
    ]
}

fn num_bpy(neg: bool, mag: String) -> String {
    if neg { format!("-{mag}") } else { mag }
}

fn bpy(v: &V) -> String {
    match v {
        V::Int(i) => num_bpy(*i < 0, i.unsigned_abs().to_string()),
        V::Float(f) => num_bpy(f.is_sign_negative(), format!("{:?}", f.abs())),
        V::Str(s) => py_str_repr(s),
        V::Enum(e) => format!("'{e}'"),
        V::Bool(b, _) => if *b { "True" } else { "False" }.into(),
        V::None(_) => "None".into(),
        V::Tuple(items) => format!("({})", items.iter().map(bpy).collect::<Vec<_>>().join(", ")),
        V::Set(items) => format!(
            "{{{}}}",
            items
                .iter()
                .map(|e| format!("'{e}'"))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn short(v: &V) -> String {
    match v {
        V::Enum(e) => e.clone(),
        V::Bool(b, lower) => match (*b, *lower) {
            (true, true) => "true".into(),
            (false, true) => "false".into(),
            (true, false) => "True".into(),
            (false, false) => "False".into(),
        },
        V::None(lower) => if *lower { "none" } else { "None" }.into(),
        V::Tuple(items) => items.iter().map(short).collect::<Vec<_>>().join(","),
        V::Set(items) => format!("{{{}}}", items.join(",")),
        other => bpy(other),
    }
}

fn erased(src: &str) -> Result<Command, String> {
    let mut v = parse_complete(src);
    if v.len() != 1 {
        return Err(format!("{} statements from {src:?}", v.len()));
    }
    match v.remove(0) {
        Ok(s) => {
            let mut c = s.cmd;
            c.erase_spans();
            normalise(&mut c);
            Ok(c)
        }
        Err(e) => Err(format!("{src:?}: {} {:?}", e.message, e.span)),
    }
}

fn normalise(c: &mut Command) {
    match c {
        Command::Invoke { args, .. } | Command::Call { args, .. } => args
            .iter_mut()
            .for_each(|a| common::bare_to_str(&mut a.value)),
        Command::Set { value, .. } => common::bare_to_str(value),
        _ => {}
    }
}

proptest! {
    #![proptest_config(cfg())]

    #[test]
    fn op_calls_lower_identically(
        ns in ident(),
        op in ident(),
        kwargs in prop::collection::btree_map(ident(), value(), 0..5),
    ) {
        let b = format!(
            "bpy.ops.{ns}.{op}({})",
            kwargs.iter().map(|(k, v)| format!("{k}={}", bpy(v))).collect::<Vec<_>>().join(", ")
        );
        let s = format!(
            "{ns}.{op}{}",
            kwargs.iter().map(|(k, v)| format!(" {k}={}", short(v))).collect::<String>()
        );
        let (a, c) = (erased(&b), erased(&s));
        prop_assert!(a.is_ok(), "{:?}", a);
        prop_assert_eq!(a, c, "{} vs {}", b, s);
    }

    #[test]
    fn assignments_lower_identically(name in "[ -~é]{0,8}", attr in ident(), v in value(), aug in 0usize..5) {
        let op = ["=", "+=", "-=", "*=", "/="][aug];
        let b = format!("bpy.data.objects[{}].{attr} {op} {}", py_str_repr(&name), bpy(&v));
        let s = format!("data.objects[{}].{attr} {op} {}", py_str_repr(&name), short(&v));
        let (a, c) = (erased(&b), erased(&s));
        prop_assert!(a.is_ok(), "{:?}", a);
        prop_assert_eq!(a, c, "{} vs {}", b, s);
    }

    #[test]
    fn rna_calls_lower_identically(
        func in ident(),
        positional in prop::collection::vec(value(), 0..3),
        kwargs in prop::collection::btree_map(ident(), value(), 0..3),
    ) {
        let mut bargs: Vec<String> = positional.iter().map(bpy).collect();
        bargs.extend(kwargs.iter().map(|(k, v)| format!("{k}={}", bpy(v))));
        let mut sargs: Vec<String> = positional.iter().map(short).collect();
        sargs.extend(kwargs.iter().map(|(k, v)| format!("{k}={}", short(v))));
        let b = format!("bpy.context.object.{func}({})", bargs.join(", "));
        // With no arguments the short form needs the Python-style parentheses.
        let s = if sargs.is_empty() {
            format!("context.object.{func}()")
        } else {
            format!("context.object.{func} {}", sargs.join(" "))
        };
        let (a, c) = (erased(&b), erased(&s));
        prop_assert!(a.is_ok(), "{:?}", a);
        prop_assert_eq!(a, c, "{} vs {}", b, s);
    }
}
