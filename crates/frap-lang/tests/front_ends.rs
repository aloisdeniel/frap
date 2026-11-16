// SPDX-License-Identifier: GPL-3.0-or-later
//! WP 1.3 acceptance (ARCHITECTURE §22.3): the §11.5 pairs, never waiting, the raw splitter,
//! §11.4 routing, `OpMeta`, and spans on every error (bytes vs code points).

mod common;

use common::{bare_to_str, err, ir};
use frap_lang::split::RawSplitter;
use frap_lang::*;
use frap_rna::{PyExc, Span};

fn lit(l: Lit) -> Expr {
    Expr::Lit(l, Span::default())
}
fn int(i: i64) -> Expr {
    lit(Lit::Int(i))
}
fn s(x: &str) -> Expr {
    lit(Lit::Str(x.into()))
}
fn bare(x: &str) -> Expr {
    Expr::Bare(x.into(), Span::default())
}
fn tuple(v: Vec<Expr>) -> Expr {
    Expr::Tuple(v, Span::default())
}
fn kw(name: &str, value: Expr) -> Arg {
    Arg {
        name: Some(name.into()),
        value,
        span: Span::default(),
    }
}
fn pos(value: Expr) -> Arg {
    Arg {
        name: None,
        value,
        span: Span::default(),
    }
}
fn path(root: PathRoot, segs: Vec<PathSeg>) -> PathExpr {
    PathExpr {
        root,
        segs,
        span: Span::default(),
    }
}
fn attr(a: &str) -> PathSeg {
    PathSeg::Attr(a.into())
}
fn key(k: &str) -> PathSeg {
    PathSeg::Key(k.into())
}

// ---------------------------------------------------------------------- §11.5 pairs

#[test]
fn pair_1_cube_add() {
    let (a, fa) = ir("bpy.ops.mesh.primitive_cube_add(size=2, location=(0, 0, 1))");
    let (b, fb) = ir("mesh.primitive_cube_add size=2 location=0,0,1");
    assert_eq!((fa, fb), (Form::Bpy, Form::Short));
    assert_eq!(a, b);
    assert_eq!(
        a,
        Command::Invoke {
            idname: "mesh.primitive_cube_add".into(),
            positional: vec![],
            args: vec![
                kw("size", int(2)),
                kw("location", tuple(vec![int(0), int(0), int(1)])),
            ],
        }
    );
}

#[test]
fn pair_2_location_set() {
    let (a, _) = ir("bpy.data.objects['Cube'].location = (1, 2, 3)");
    let (b, fb) = ir("data.objects['Cube'].location = 1,2,3");
    assert_eq!(fb, Form::Short);
    assert_eq!(a, b);
    assert_eq!(
        a,
        Command::Set {
            target: path(
                PathRoot::BpyData,
                vec![attr("objects"), key("Cube"), attr("location")]
            ),
            op: AssignOp::Assign,
            value: tuple(vec![int(1), int(2), int(3)]),
        }
    );
}

#[test]
fn pair_3_macro_sub_operator() {
    let (a, _) =
        ir(r#"bpy.ops.mesh.extrude_region_move(TRANSFORM_OT_translate={"value": (0, 0, 1)})"#);
    let (b, _) = ir("mesh.extrude_region_move TRANSFORM_OT_translate.value=0,0,1");
    assert_eq!(a, b);
    assert_eq!(
        a,
        Command::Invoke {
            idname: "mesh.extrude_region_move".into(),
            positional: vec![],
            args: vec![kw(
                "TRANSFORM_OT_translate",
                Expr::Dict(
                    vec![(s("value"), tuple(vec![int(0), int(0), int(1)]))],
                    Span::default()
                )
            )],
        }
    );
    // Several dotted keys merge into one dict per sub-operator, in source order.
    let (a, _) = ir(
        "bpy.ops.mesh.extrude_region_move(MESH_OT_extrude_region={'use_normal_flip': False}, \
         TRANSFORM_OT_translate={'value': (0, 0, 1), 'orient_type': 'NORMAL'})",
    );
    let (b, _) = ir(
        "mesh.extrude_region_move MESH_OT_extrude_region.use_normal_flip=false \
         TRANSFORM_OT_translate.value=0,0,1 TRANSFORM_OT_translate.orient_type='NORMAL'",
    );
    assert_eq!(a, b);
}

#[test]
fn pair_4_enum_quoted_or_bare() {
    let (a, _) = ir("bpy.ops.mesh.select_mode(type='FACE')");
    let (b, _) = ir("mesh.select_mode type=FACE");
    // The IR keeps the difference (Lit::Str vs Bare); coerce maps both to the same
    // Value::Enum (§11.5). With Bare read as Str they are identical...
    let (Command::Invoke { args: mut ab, .. }, Command::Invoke { args: aa, .. }) = (b, a.clone())
    else {
        panic!("not invokes")
    };
    assert_eq!(ab[0].value, bare("FACE"));
    bare_to_str(&mut ab[0].value);
    assert_eq!(aa, ab);
    // ... and quoting in the short form gives literally the same IR.
    let (c, _) = ir("mesh.select_mode type='FACE'");
    assert_eq!(a, c);
}

#[test]
fn typed_short_values_lower_like_bpy() {
    let pairs = [
        (
            "bpy.ops.transform.translate(value=(-1.5, 2e1, +3))",
            "transform.translate value=-1.5,2e1,+3",
        ),
        (
            "bpy.ops.object.delete(use_global=False, confirm=True)",
            "object.delete use_global=false confirm=True",
        ),
        (
            "bpy.ops.frap.render_contact_sheet(frame=None, views={'FRONT', 'TOP'})",
            "frap.render_contact_sheet frame=none views={'FRONT','TOP'}",
        ),
        (
            "bpy.ops.export_scene.gltf(filepath='out/part.glb')",
            "export_scene.gltf filepath='out/part.glb'",
        ),
        (
            "bpy.ops.mesh.primitive_cube_add(rotation=(0, 0, radians(90)))",
            "mesh.primitive_cube_add rotation=(0, 0, radians(90))",
        ),
        (
            "bpy.ops.mesh.primitive_cube_add(rotation=(0, 0, radians(90)))",
            "mesh.primitive_cube_add rotation=0,0,radians(90)",
        ),
        (
            "bpy.ops.transform.translate(value=Vector((0, 0, 1)))",
            "transform.translate value=Vector((0,0,1))",
        ),
        (
            "bpy.ops.object.parent_set(target=bpy.data.objects['Empty'])",
            "object.parent_set target=data.objects['Empty']",
        ),
        (
            "bpy.data.objects['Cube'].select_set(True)",
            "data.objects['Cube'].select_set True",
        ),
        (
            "bpy.data.objects['Cube'].select_set(True)",
            "D.objects['Cube'].select_set True",
        ),
        (
            "bpy.data.objects.new('Thing', object_data=bpy.data.meshes['Cube'])",
            "data.objects.new 'Thing' object_data=data.meshes['Cube']",
        ),
        (
            "bpy.data.objects['Cube'].location += (0, 0, 1)",
            "data.objects['Cube'].location += 0,0,1",
        ),
        (
            "bpy.data.objects['Cube'].location = 1, 2, 3",
            "D.objects['Cube'].location = 1,2,3",
        ),
    ];
    for (b, sh) in pairs {
        assert_eq!(ir(b).0, ir(sh).0, "{b}  vs  {sh}");
    }
}

// ---------------------------------------------------------------------- never waits

#[test]
fn parse_complete_never_waits() {
    for (src, msg) in [
        (
            "bpy.ops.mesh.primitive_cube_add(size=2",
            "'(' was never closed",
        ),
        ("bpy.data.objects['Cube'", "'[' was never closed"),
        ("print({'a': 1", "'{' was never closed"),
        (
            "bpy.ops.mesh.primitive_cube_add(location=(0, 0, [1",
            "'[' was never closed",
        ),
        (
            "print('''abc",
            "unterminated triple-quoted string literal (detected at line 1)",
        ),
        (
            "print('''abc\ndef",
            "unterminated triple-quoted string literal (detected at line 2)",
        ),
        ("bpy.data.objects \\\n", "unexpected EOF while parsing"),
    ] {
        let e = err(src);
        assert_eq!(e.message, msg, "{src:?}");
        assert_eq!(e.exception, PyExc::SyntaxError);
        assert_eq!(e.code.0, "SYNTAX");
        assert!(e.span.is_some());
    }
    // The bracket position is reported (CPython's offset, 1-based).
    let e = err("bpy.ops.mesh.primitive_cube_add(size=2");
    let sp = e.span.expect("span");
    assert_eq!((sp.line, sp.col, sp.col_chars, sp.len), (1, 32, 32, 1));
    // The error consumes the rest of the unit; nothing hangs and later statements of the
    // unit are swallowed exactly as CPython does.
    let v = parse_complete("bpy.data.objects\nbpy.ops.x.y(\nbpy.data.objects");
    assert_eq!(v.len(), 2);
    assert!(v[0].is_ok() && v[1].is_err());
}

#[test]
fn raw_splitter_ends_continuations_on_blank_line_and_eof() {
    let mut r = RawSplitter::new();
    let mut out = Vec::new();
    let input = "bpy.ops.mesh.primitive_cube_add(\n  size=2)\n\
                 bpy.ops.mesh.primitive_cube_add(size=2\n\
                 \n\
                 bpy.data.objects\n\
                 print('''a\n\
                 b''')\n\
                 bpy.data.objects['Cube'].location = \\\n  (1, 2, 3)\n\
                 bpy.ops.mesh.primitive_cube_add(location=(0,\n";
    for line in input.split_inclusive('\n') {
        if let Some(c) = r.push_line(line) {
            out.push(c);
        }
    }
    if let Some(c) = r.finish() {
        out.push(c);
    }
    let firsts: Vec<u32> = out.iter().map(|c| c.first_line).collect();
    assert_eq!(firsts, vec![1, 3, 5, 6, 8, 10]);
    let results: Vec<Vec<Result<Stmt, _>>> = out
        .iter()
        .map(|c| parse_complete_at(&c.src, c.first_line))
        .collect();
    assert!(results[0][0].is_ok());
    // Blank line ended the continuation: an immediate SyntaxError, on the right line.
    let e = results[1][0].as_ref().unwrap_err();
    assert_eq!(e.message, "'(' was never closed");
    assert_eq!(e.span.map(|s| s.line), Some(3));
    assert!(results[2][0].is_ok());
    assert!(results[3][0].is_ok(), "triple-quoted string continues");
    assert!(results[4][0].is_ok(), "backslash continuation continues");
    assert_eq!(results[4][0].as_ref().map(|s| s.line).ok(), Some(8));
    // EOF ended the last one.
    let e = results[5][0].as_ref().unwrap_err();
    assert_eq!(e.message, "'(' was never closed");
    assert_eq!(e.span.map(|s| (s.line, s.col)), Some((10, 42)));
}

// ---------------------------------------------------------------------- §11.4 routing

#[test]
fn path_versus_bare_rule() {
    // A word is a path only when a root is immediately followed by '.' or '['.
    let (c, _) = ir("object.select_pattern pattern=data");
    let Command::Invoke { args, .. } = c else {
        panic!()
    };
    assert_eq!(args[0].value, bare("data"));
    let (c, _) = ir("object.select_pattern pattern=context");
    let Command::Invoke { args, .. } = c else {
        panic!()
    };
    assert_eq!(args[0].value, bare("context"));
    let (c, _) = ir("object.parent_set target=data.objects['Cube']");
    let Command::Invoke { args, .. } = c else {
        panic!()
    };
    assert_eq!(
        args[0].value,
        Expr::Path(path(PathRoot::BpyData, vec![attr("objects"), key("Cube")]))
    );
    // BARE never swallows the '['; a dotted word stays one BARE.
    let (c, _) = ir("data.objects['Cube'].name = Cube.001");
    let Command::Set { value, .. } = c else {
        panic!()
    };
    assert_eq!(value, bare("Cube.001"));
    let (c, _) = ir("object.select_pattern pattern=my-object.001a");
    let Command::Invoke { args, .. } = c else {
        panic!()
    };
    assert_eq!(args[0].value, bare("my-object.001a"));
    // `data`/`context` statements are short paths; other `ident.ident` are op calls.
    assert_eq!(ir("data.objects").1, Form::Short);
    assert_eq!(ir("context.object").1, Form::Short);
    let (c, f) = ir("foo.bar baz=1");
    assert_eq!(f, Form::Short);
    assert!(matches!(c, Command::Invoke { idname, .. } if idname == "foo.bar"));
}

#[test]
fn d_and_c_routing() {
    // End of statement, '=', augmented ops, '(' → bpy form.
    for src in [
        "D.objects['Cube'].location",
        "D.objects['Cube'].location = (1, 2, 3)",
        "C.scene.frame_current += 1",
        "D.objects['Cube'].select_set(True)",
        "D.objects['Cube'].select_set (True)",
        "C",
    ] {
        assert_eq!(ir(src).1, Form::Bpy, "{src}");
    }
    // Whitespace followed by a value or key=value → short form call.
    for src in [
        "D.objects['Cube'].select_set True",
        "C.object.modifiers.new name=Bevel type=BEVEL",
        "D.materials.new Steel",
        "D.materials.new 'Steel'",
        "C.scene.frame_set -1",
        "D.objects ['Cube']",
    ] {
        let (c, f) = ir(src);
        assert_eq!(f, Form::Short, "{src}");
        assert!(matches!(c, Command::Call { .. }), "{src}");
    }
    let (c, _) = ir("C.object.modifiers.new name=Bevel type=BEVEL");
    assert_eq!(
        c,
        Command::Call {
            target: path(
                PathRoot::BpyContext,
                vec![attr("object"), attr("modifiers"), attr("new")]
            ),
            args: vec![kw("name", bare("Bevel")), kw("type", bare("BEVEL"))],
        }
    );
    // `D = 1` binds a name: unsupported.
    assert_eq!(err("D = 1").code.0, "UNSUPPORTED_SYNTAX");
    // In the bpy form an unquoted word is a Python name.
    let e = err("D.objects['Cube'].name = Cube");
    assert_eq!((e.exception, e.code.0), (PyExc::NameError, "NAME"));
    assert_eq!(e.message, "name 'Cube' is not defined");
}

#[test]
fn rna_calls_and_nesting() {
    let (c, _) = ir(r#"bpy.context.object.data.materials.append(bpy.data.materials.new("Red"))"#);
    assert_eq!(
        c,
        Command::Call {
            target: path(
                PathRoot::BpyContext,
                vec![
                    attr("object"),
                    attr("data"),
                    attr("materials"),
                    attr("append")
                ]
            ),
            args: vec![pos(Expr::Path(path(
                PathRoot::BpyData,
                vec![
                    attr("materials"),
                    attr("new"),
                    PathSeg::Call(vec![pos(s("Red"))])
                ]
            )))],
        }
    );
    // The Call target's span ends at the function name.
    let st = parse_complete("bpy.data.materials.new('Red')")
        .remove(0)
        .expect("ok");
    let Command::Call { target, .. } = st.cmd else {
        panic!()
    };
    assert_eq!(target.span.len as usize, "bpy.data.materials.new".len());
    // A call in the middle of a path stays a segment of a read.
    let (c, _) = ir("bpy.data.materials.new('Red').name");
    assert!(matches!(c, Command::Get { target } if target.segs.len() == 4));
    // (name, library) keys.
    let (c, _) = ir("bpy.data.node_groups['Smooth by Angle', None]");
    let Command::Get { target } = c else { panic!() };
    assert_eq!(
        target.segs[1],
        PathSeg::KeyLib("Smooth by Angle".into(), None)
    );
}

#[test]
fn op_meta_queries() {
    for (src, what) in [
        ("bpy.ops.mesh.bevel.poll()", OpMetaKind::Poll),
        ("bpy.ops.mesh.bevel.get_rna_type()", OpMetaKind::GetRnaType),
        ("bpy.ops.mesh.bevel.idname()", OpMetaKind::Idname),
        ("bpy.ops.mesh.bevel.idname_py()", OpMetaKind::IdnamePy),
    ] {
        let (c, f) = ir(src);
        assert_eq!(f, Form::Bpy);
        assert_eq!(
            c,
            Command::OpMeta {
                idname: "mesh.bevel".into(),
                what
            }
        );
    }
    let (c, _) = ir("bpy.ops.x.y.poll()");
    assert_eq!(
        c,
        Command::OpMeta {
            idname: "x.y".into(),
            what: OpMetaKind::Poll
        }
    );
    assert_eq!(
        err("bpy.ops.mesh.bevel.bl_rna").code.0,
        "UNSUPPORTED_SYNTAX"
    );
}

#[test]
fn statements_of_every_kind() {
    assert!(matches!(
        ir("print(1, 'a')").0,
        Command::Print { values } if values.len() == 2
    ));
    for src in [
        "import bpy",
        "import bpy, math, mathutils, bmesh",
        "from mathutils import Vector, Matrix",
        "from math import *",
    ] {
        assert_eq!(
            ir(src).0,
            Command::Noop {
                reason: NoopReason::Import
            }
        );
    }
    assert_eq!(
        ir("pass").0,
        Command::Noop {
            reason: NoopReason::Pass
        }
    );
    // `;` and newlines separate statements; blank and comment-only pieces are dropped.
    let v = parse_complete("import bpy; bpy.data.objects\n\n# x\nmesh.primitive_cube_add");
    assert_eq!(v.len(), 3);
    assert_eq!(
        v.iter()
            .map(|r| r.as_ref().map(|s| s.line).unwrap_or(0))
            .collect::<Vec<_>>(),
        vec![1, 1, 4]
    );
}

#[test]
fn unsupported_python_is_rejected_with_the_right_code() {
    let e = err("cube = bpy.data.objects['Cube']");
    assert_eq!(
        (e.exception, e.code.0),
        (PyExc::SyntaxError, "UNSUPPORTED_SYNTAX")
    );
    assert_eq!(
        e.message,
        "frap: variables are not supported; use a full path such as bpy.data.objects['Cube']"
    );
    assert_eq!(
        e.hint.as_deref(),
        Some("use a full path such as bpy.data.objects['Cube']")
    );
    for src in [
        "for o in bpy.data.objects: pass",
        "while True: pass",
        "if True: pass",
        "def f(): pass",
        "class A: pass",
        "with x: pass",
        "print(lambda: 1)",
        "print([o for o in bpy.data.objects])",
        "print(f'{1}')",
    ] {
        assert_eq!(err(src).code.0, "UNSUPPORTED_SYNTAX", "{src}");
    }
    // Python's own wording for real syntax errors.
    for (src, msg) in [
        (
            "bpy.ops.mesh.primitive_cube_add(size=2, size=3)",
            "keyword argument repeated: size",
        ),
        (
            "bpy.ops.mesh.primitive_cube_add(size=2, (0, 0, 1))",
            "positional argument follows keyword argument",
        ),
        (
            "bpy.ops.mesh.primitive_cube_add(size=2 location=(0, 0, 1))",
            "invalid syntax. Perhaps you forgot a comma?",
        ),
        (
            "bpy.data.objects['Cube'].select_get() = True",
            "cannot assign to function call here. Maybe you meant '==' instead of '='?",
        ),
        ("bpy.ops.mesh.primitive_cube_add(size=2))", "unmatched ')'"),
        (
            "bpy.ops.mesh.primitive_cube_add(size=2]",
            "closing parenthesis ']' does not match opening parenthesis '('",
        ),
        (
            "print('abc",
            "unterminated string literal (detected at line 1)",
        ),
    ] {
        let e = err(src);
        assert_eq!((e.code.0, e.message.as_str()), ("SYNTAX", msg), "{src}");
    }
}

// ---------------------------------------------------------------------- spans

#[test]
fn every_error_has_a_span_with_bytes_and_code_points() {
    // 'Würfel' has a 2-byte 'ü': after it, byte columns run one ahead of code points.
    let e = err("bpy.data.objects['Würfel'].location = (1, 2");
    let sp = e.span.expect("span");
    assert_eq!(e.message, "'(' was never closed");
    assert_eq!((sp.line, sp.col, sp.col_chars, sp.len), (1, 40, 39, 1));
    assert_eq!(sp.start, 39);

    let e = err("data.objects['Würfel'].location = 1,,2");
    let sp = e.span.expect("span");
    assert_eq!((sp.col, sp.col_chars), (38, 37));

    // Emoji: 4 bytes, 1 code point.
    let e = err("bpy.data.objects['🧊'].name = 🧊");
    let sp = e.span.expect("span");
    assert_eq!(e.message, "invalid character '🧊' (U+1F9CA)");
    assert_eq!((sp.col, sp.col_chars, sp.len), (33, 30, 4));

    // Second line of a unit: columns restart, start stays a unit offset.
    let src = "bpy.data.objects\n  bpy.data.objects['é'] = )";
    let v = parse_complete(src);
    let e = v[1].as_ref().unwrap_err();
    let sp = e.span.expect("span");
    assert_eq!((sp.line, sp.col, sp.col_chars), (2, 28, 27));
    assert_eq!(&src[sp.start as usize..sp.start as usize + 1], ")");

    // Arguments have spans too (the executor points at the keyword, §13.5).
    let st = parse_complete("bpy.ops.mesh.primitive_cube_add(sizee=2)")
        .remove(0)
        .expect("ok");
    let Command::Invoke { args, .. } = st.cmd else {
        panic!()
    };
    assert_eq!((args[0].span.col, args[0].span.col_chars), (33, 33));
}

#[test]
fn spans_respect_first_line() {
    let v = parse_complete_at("bpy.data.objects\nbpy.ops.x.y(", 41);
    assert_eq!(v[0].as_ref().map(|s| s.line).ok(), Some(41));
    let e = v[1].as_ref().unwrap_err();
    assert_eq!(e.span.map(|s| (s.line, s.col)), Some((42, 12)));
}

#[test]
fn error_codes_are_registered_for_describe() {
    for (code, exc) in [
        (frap_lang::errors::SYNTAX, PyExc::SyntaxError),
        (frap_lang::errors::UNSUPPORTED_SYNTAX, PyExc::SyntaxError),
        (frap_lang::errors::NAME, PyExc::NameError),
    ] {
        let doc = frap_rna::ERROR_CODES
            .iter()
            .find(|d| d.code == code)
            .unwrap_or_else(|| panic!("{} is not registered", code.0));
        assert_eq!(doc.exception, exc);
    }
}
