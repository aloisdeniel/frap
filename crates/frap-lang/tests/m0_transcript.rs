// SPDX-License-Identifier: GPL-3.0-or-later
//! The M0 exit transcript through the parser (ARCHITECTURE §11.5, §22.2).

use frap_lang::*;

fn one(src: &str) -> Stmt {
    let mut v = parse_complete(src);
    assert_eq!(v.len(), 1, "{src}");
    v.remove(0)
        .unwrap_or_else(|e| panic!("{src}: {e} {:?}", e.hint))
}

fn lit_int(i: i64) -> Expr {
    Expr::Lit(Lit::Int(i), Default::default())
}

#[test]
fn both_cube_statements_lower_to_the_same_ir() {
    let bpy = one("bpy.ops.mesh.primitive_cube_add(size=2, location=(0, 0, 1))");
    let short = one("mesh.primitive_cube_add size=2 location=0,0,1");
    assert_eq!(bpy.form, Form::Bpy);
    assert_eq!(short.form, Form::Short);
    let (mut a, mut b) = (bpy.cmd, short.cmd);
    a.erase_spans();
    b.erase_spans();
    assert_eq!(a, b);
    // ... and it is exactly the Command of §11.5.
    let expected = Command::Invoke {
        idname: "mesh.primitive_cube_add".into(),
        positional: vec![],
        args: vec![
            Arg {
                name: Some("size".into()),
                value: lit_int(2),
                span: Default::default(),
            },
            Arg {
                name: Some("location".into()),
                value: Expr::Tuple(vec![lit_int(0), lit_int(0), lit_int(1)], Default::default()),
                span: Default::default(),
            },
        ],
    };
    assert_eq!(a, expected);
}

#[test]
fn set_and_get_paths() {
    let mut s = one("bpy.data.objects['Cube'].location = (1, 2, 3)").cmd;
    s.erase_spans();
    let target = PathExpr {
        root: PathRoot::BpyData,
        segs: vec![
            PathSeg::Attr("objects".into()),
            PathSeg::Key("Cube".into()),
            PathSeg::Attr("location".into()),
        ],
        span: Default::default(),
    };
    assert_eq!(
        s,
        Command::Set {
            target: target.clone(),
            op: AssignOp::Assign,
            value: Expr::Tuple(vec![lit_int(1), lit_int(2), lit_int(3)], Default::default())
        }
    );
    let mut short = one("data.objects['Cube'].location = 1,2,3").cmd;
    short.erase_spans();
    assert_eq!(s, short);
    let mut g = one("bpy.data.objects['Cube'].location").cmd;
    g.erase_spans();
    assert_eq!(
        g,
        Command::Get {
            target: target.clone()
        }
    );
    let mut d = one("D.objects['Cube'].location").cmd;
    d.erase_spans();
    assert_eq!(d, Command::Get { target });
}

#[test]
fn export_statement() {
    let mut s = one("bpy.ops.export_scene.gltf(filepath='out/m0.glb')").cmd;
    s.erase_spans();
    assert_eq!(
        s,
        Command::Invoke {
            idname: "export_scene.gltf".into(),
            positional: vec![],
            args: vec![Arg {
                name: Some("filepath".into()),
                value: Expr::Lit(Lit::Str("out/m0.glb".into()), Default::default()),
                span: Default::default()
            }],
        }
    );
}

#[test]
fn whole_transcript_lines_and_spans() {
    let src = "bpy.ops.mesh.primitive_cube_add(size=2, location=(0, 0, 1))\n\
               mesh.primitive_cube_add size=2 location=0,0,1\n\
               # a comment\n\
               bpy.data.objects['Cube'].location = (1, 2, 3)\n\
               bpy.data.objects['Cube'].location\n\
               bpy.ops.export_scene.gltf(filepath='out/m0.glb')\n";
    let v: Vec<_> = parse_complete(src)
        .into_iter()
        .map(Result::unwrap)
        .collect();
    assert_eq!(
        v.iter().map(|s| s.line).collect::<Vec<_>>(),
        vec![1, 2, 4, 5, 6]
    );
    // Span of `size=2` in the short form: line 2, col 25.
    if let Command::Invoke { args, .. } = &v[1].cmd {
        assert_eq!(
            (args[0].span.line, args[0].span.col, args[0].span.len),
            (2, 25, 6)
        );
    } else {
        panic!()
    }
}

#[test]
fn bracket_continuation_and_errors() {
    let v = parse_complete("bpy.ops.mesh.primitive_cube_add(\n  size=2,\n  location=(0, 0, 1))");
    assert_eq!(v.len(), 1);
    assert!(v[0].is_ok());
    let e = parse_complete("bpy.ops.mesh.primitive_cube_add(size=2")
        .remove(0)
        .unwrap_err();
    assert_eq!(e.message, "'(' was never closed");
    assert_eq!(e.code.0, "SYNTAX");
    let e = parse_complete("cube = 1").remove(0).unwrap_err();
    assert_eq!(e.code.0, "UNSUPPORTED_SYNTAX");
    assert!(!is_complete("bpy.ops.mesh.primitive_cube_add(size=2,"));
    assert!(is_complete("bpy.ops.mesh.primitive_cube_add(size=2)"));
    assert_eq!(parse_complete("import bpy; bpy.data.objects").len(), 2);
}

#[test]
fn short_form_bare_words_and_negatives() {
    let mut s = one("mesh.primitive_cube_add align=WORLD location=0,-1.5,2e1").cmd;
    s.erase_spans();
    let Command::Invoke { args, .. } = s else {
        panic!()
    };
    assert_eq!(
        args[0].value,
        Expr::Bare("WORLD".into(), Default::default())
    );
    assert_eq!(
        args[1].value,
        Expr::Tuple(
            vec![
                lit_int(0),
                Expr::Unary(
                    UnOp::Neg,
                    Box::new(Expr::Lit(Lit::Float(1.5), Default::default())),
                    Default::default()
                ),
                Expr::Lit(Lit::Float(20.0), Default::default())
            ],
            Default::default()
        )
    );
    let mut s = one("data.objects['Cube'].name = Cube.001").cmd;
    s.erase_spans();
    let Command::Set { value, .. } = s else {
        panic!()
    };
    assert_eq!(value, Expr::Bare("Cube.001".into(), Default::default()));
}
