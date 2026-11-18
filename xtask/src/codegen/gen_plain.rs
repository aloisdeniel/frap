// SPDX-License-Identifier: GPL-3.0-or-later
//! `crates/frap-scene/src/generated/**` (ARCHITECTURE §4.2): plain data structs whose `Default`
//! is the dumped fresh value (RNA default overlaid with `fresh.differs_from_default`; read-only
//! props ignored, RFC 0001 F-DEF-7), with serde and generated RNA accessors, plus
//! `ModifierType` / `ModifierSettings` (one settings struct per mesh modifier type, `Default` =
//! `modifiers.new` values from `spec/blender-5.2/modifiers/<TYPE>.json`).
//!
//! Field types follow the M1 W0 stand-ins (decision W0-21): BOOLEAN → `bool`, INT → `i32`,
//! FLOAT → `f32` (`f64` with the float-storage overlay), fixed arrays → `[T; N]`, matrices →
//! `[[f32; C]; R]`, STRING → `String`, ENUM → `SmolStr` (identifier; validated by `coerce` on
//! write), ENUM_FLAG → `BTreeSet<SmolStr>`, ID pointers → `Option<IdAny>` (not serialized: the
//! `.frap` scene map writes ID references by name), pointers to another plain struct → that
//! struct. Dynamic arrays, collections and other pointers are not stored here (their RNA reads
//! answer `Field::NeedsHook`).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use serde_json::Value as J;

use super::Files;
use super::model::{Def, EnumDef, Kind, Prop, Res, Shape, Spec, Struct, jf64};
use super::rust::{camel, header, ident, lit_f32, lit_f64, lit_str, screaming, snake};

pub const DIR: &str = "crates/frap-scene/src/generated";

/// One plain struct.
struct PlainSpec {
    /// Rust name.
    rust: &'static str,
    /// Struct(s) whose props are fields. More than one = a union over subclasses (Light).
    srnas: &'static [&'static str],
    /// Only these props (ToolSettings subset); `None` = all.
    include: Option<&'static [&'static str]>,
    /// Props bound elsewhere (hooks).
    exclude: &'static [&'static str],
    /// Only the struct's own props (the ID base is bound by the owning datablock).
    own_only: bool,
    /// Implements `Rna`/`RnaMut` (`false` for the field sets of an ID, which its binding wraps).
    rna_impl: bool,
    /// Stored props whose RNA get/set go through registered hooks (`Field::NeedsHook`): the
    /// field is DNA-like storage the hook routes to (`View3DShading.studio_light`).
    hooked: &'static [&'static str],
    /// Stored `SmolStr` fields that are no RNA property (DNA storage only a hook reads).
    extra: &'static [Extra],
    doc: &'static str,
}

/// A stored field without an RNA property: a `SmolStr` (`ty: None`, default `default`), or a
/// hand-written type of `frap-scene` (`ty: Some(path)`, `Default::default()`; M4 W0: the
/// `NodesModifier.properties.inputs` storage).
struct Extra {
    name: &'static str,
    doc: &'static str,
    default: &'static str,
    ty: Option<&'static str>,
}

/// Stored fields of generated modifier settings structs that are no RNA property, by struct.
const MODIFIER_EXTRAS: &[(&str, &[Extra])] = &[(
    "NodesModifier",
    &[Extra {
        name: "inputs",
        doc: "`properties.inputs`: the group inputs by identifier (5.2 runtime-defined RNA `NodesModifierProperties`, bound by `frap-scene/src/id/modifier.rs`, WP 4.1)",
        default: "",
        ty: Some("crate::id::GnInputs"),
    }],
)];

/// RNA properties of generated modifier settings structs that are another name for a stored
/// field (Blender defines both on one DNA member), as `(struct, &[(alias, target)])`: the alias
/// gets no field, and reading or writing it reads or writes the target.
/// `BevelModifier.width_pct` and `width` are both `BevelModifierData::value` (`rna_modifier.cc`).
const MODIFIER_ALIASES: &[(&str, &[(&str, &str)])] =
    &[("BevelModifier", &[("width_pct", "width")])];

const PLAIN: &[PlainSpec] = &[
    PlainSpec {
        rust: "View3DCursor",
        srnas: &["View3DCursor"],
        include: None,
        exclude: &[],
        own_only: false,
        rna_impl: true,
        hooked: &[],
        extra: &[],
        doc: "`bpy.types.View3DCursor` (`scene.cursor`).",
    },
    PlainSpec {
        rust: "ToolSettings",
        srnas: &["ToolSettings"],
        include: Some(&[
            "mesh_select_mode",
            "transform_pivot_point",
            "use_proportional_edit",
            "use_proportional_edit_objects",
            "proportional_edit_falloff",
            "proportional_size",
            "use_proportional_connected",
            "use_proportional_projected",
            "double_threshold",
            "use_uv_select_sync",
            "vertex_group_weight",
            "use_transform_data_origin",
            "use_transform_pivot_point_align",
            "use_transform_skip_children",
            "use_snap",
            "snap_elements",
            "snap_target",
            "use_mesh_automerge",
            // M4 W0 (M2 leftover): `T_AUTOSPLIT` of edit-mode transforms.
            "use_mesh_automerge_and_split",
            "use_keyframe_insert_auto",
            "keyframe_type",
        ]),
        exclude: &[],
        own_only: false,
        rna_impl: true,
        hooked: &[],
        extra: &[],
        doc: "`bpy.types.ToolSettings` (`scene.tool_settings`; the OPERATORS.md §D subset).",
    },
    PlainSpec {
        rust: "View3DShading",
        srnas: &["View3DShading"],
        include: None,
        exclude: &[],
        own_only: false,
        rna_impl: true,
        // `rna_View3DShading_studio_light_get/set`: one enum over three DNA names, routed by
        // `type` and `light` (frap-scene `hooks/view3_d_shading_studio_light.rs`, WP 2.3).
        hooked: &["studio_light"],
        extra: &[
            Extra {
                name: "matcap",
                doc: "DNA `matcap`: the MatCap `studio_light` shows when `type == 'SOLID'` and `light == 'MATCAP'` (no RNA property; empty = Blender's default matcap)",
                default: "",
                ty: None,
            },
            Extra {
                name: "lookdev_light",
                doc: "DNA `lookdev_light`: the HDRI `studio_light` shows when `type != 'SOLID'` (no RNA property; empty = `forest.exr`)",
                default: "",
                ty: None,
            },
        ],
        doc: "`bpy.types.View3DShading` (`scene.display.shading`, the virtual 3D View's shading).",
    },
    PlainSpec {
        rust: "ColorManagedViewSettings",
        srnas: &["ColorManagedViewSettings"],
        include: None,
        exclude: &[],
        own_only: false,
        rna_impl: true,
        hooked: &[],
        extra: &[],
        doc: "`bpy.types.ColorManagedViewSettings` (`scene.view_settings`).",
    },
    PlainSpec {
        rust: "ColorManagedDisplaySettings",
        srnas: &["ColorManagedDisplaySettings"],
        include: None,
        exclude: &[],
        own_only: false,
        rna_impl: true,
        hooked: &[],
        extra: &[],
        doc: "`bpy.types.ColorManagedDisplaySettings` (`scene.display_settings`).",
    },
    PlainSpec {
        rust: "ColorManagedInputColorspaceSettings",
        srnas: &["ColorManagedInputColorspaceSettings"],
        include: None,
        exclude: &[],
        own_only: false,
        rna_impl: true,
        hooked: &[],
        extra: &[],
        doc: "`bpy.types.ColorManagedInputColorspaceSettings` (`image.colorspace_settings`).",
    },
    PlainSpec {
        rust: "ImageFormatSettings",
        srnas: &["ImageFormatSettings"],
        include: None,
        exclude: &[],
        own_only: false,
        rna_impl: true,
        hooked: &[],
        extra: &[],
        doc: "`bpy.types.ImageFormatSettings` (`scene.render.image_settings`).",
    },
    PlainSpec {
        rust: "RenderSettings",
        srnas: &["RenderSettings"],
        include: None,
        exclude: &[],
        own_only: false,
        rna_impl: true,
        hooked: &[],
        extra: &[],
        doc: "`bpy.types.RenderSettings` (`scene.render`).",
    },
    PlainSpec {
        rust: "SceneDisplay",
        srnas: &["SceneDisplay"],
        include: None,
        exclude: &[],
        own_only: false,
        rna_impl: true,
        hooked: &[],
        extra: &[],
        doc: "`bpy.types.SceneDisplay` (`scene.display`: Workbench render settings, incl. `shading`).",
    },
    PlainSpec {
        rust: "SceneEEVEE",
        srnas: &["SceneEEVEE"],
        include: None,
        exclude: &[],
        own_only: false,
        rna_impl: true,
        hooked: &[],
        extra: &[],
        doc: "`bpy.types.SceneEEVEE` (`scene.eevee`).",
    },
    PlainSpec {
        rust: "UnitSettings",
        srnas: &["UnitSettings"],
        include: None,
        exclude: &[],
        own_only: false,
        rna_impl: true,
        hooked: &[],
        extra: &[],
        doc: "`bpy.types.UnitSettings` (`scene.unit_settings`).",
    },
    PlainSpec {
        rust: "CameraDOFSettings",
        srnas: &["CameraDOFSettings"],
        include: None,
        exclude: &[],
        own_only: false,
        rna_impl: true,
        hooked: &[],
        extra: &[],
        doc: "`bpy.types.CameraDOFSettings` (`camera.dof`).",
    },
    PlainSpec {
        rust: "CameraFields",
        srnas: &["Camera"],
        include: None,
        exclude: &["angle", "angle_x", "angle_y"],
        own_only: true,
        rna_impl: false,
        hooked: &[],
        extra: &[],
        doc: "The plain fields of `bpy.types.Camera` (`Default` = `bpy.data.cameras.new()`). The\n/// derived `angle`/`angle_x`/`angle_y` are hooks over `lens` and the sensor (§5.6); the ID\n/// binding (`frap-scene/src/id/camera.rs`) delegates the other props to `rna_get`/`rna_set`.",
    },
    PlainSpec {
        rust: "TextureFields",
        srnas: &["Texture", "ImageTexture"],
        include: None,
        exclude: &["type", "node_tree"],
        own_only: true,
        rna_impl: false,
        hooked: &[],
        extra: &[],
        doc: "The plain fields of `bpy.types.Texture` and `ImageTexture` (M4 W0: what DISPLACE reads;\n/// `Default` = `bpy.data.textures.new(name, 'IMAGE')`). One struct holds the union, as for\n/// `LightFields`; the ID binding (`frap-scene/src/id/texture.rs`, WP 4.2) exposes the fields of\n/// the current `type` and delegates to `rna_get`/`rna_set`.",
    },
    PlainSpec {
        rust: "LightFields",
        srnas: &["Light", "PointLight", "SunLight", "SpotLight", "AreaLight"],
        include: None,
        exclude: &["type"],
        own_only: true,
        rna_impl: false,
        hooked: &[],
        extra: &[],
        doc: "The plain fields of `bpy.types.Light` and its subclasses (`PointLight`, `SunLight`,\n/// `SpotLight`, `AreaLight`). One struct holds the union, as Blender's DNA does, so re-typing a\n/// light keeps every value; RNA exposes only the fields of the current type (`Light::srna()`\n/// refines by `type`). `Default` = `lights.new(…, 'POINT')`; each subclass's own fields take\n/// that subclass's fresh value (`lights.new(…, type)`).",
    },
];

/// Rust type of a stored field.
#[derive(Clone, Debug)]
enum FTy {
    Bool,
    I32,
    F32,
    F64,
    Arr(&'static str, u16),
    Mat(u16, u16),
    Str,
    Enum,
    EnumSet,
    Id,
    Nested(String),
}

impl FTy {
    fn rust(&self) -> String {
        match self {
            FTy::Bool => "bool".into(),
            FTy::I32 => "i32".into(),
            FTy::F32 => "f32".into(),
            FTy::F64 => "f64".into(),
            FTy::Arr(t, n) => format!("[{t}; {n}]"),
            FTy::Mat(r, c) => format!("[[f32; {c}]; {r}]"),
            FTy::Str => "String".into(),
            FTy::Enum => "SmolStr".into(),
            FTy::EnumSet => "BTreeSet<SmolStr>".into(),
            FTy::Id => "Option<IdAny>".into(),
            FTy::Nested(n) => n.clone(),
        }
    }
}

struct Field {
    prop: Prop,
    ty: FTy,
    /// Struct whose fresh value seeds the default.
    srna: String,
}

fn field_type(p: &Prop, plain_by_srna: &BTreeMap<&str, &str>) -> Option<FTy> {
    if p.has_flag("PYTHON_ONLY") || p.has_flag("CONTEXT_MEMBER") {
        return None;
    }
    // A nested settings struct is a READONLY pointer (it cannot be re-pointed) whose content is
    // writable: it is stored. Every other read-only prop is computed (hook) and not stored.
    let nested_plain = matches!(&p.kind, Kind::Pointer { is_id: false, fixed_type } if plain_by_srna.contains_key(fixed_type.as_str()));
    if p.has_flag("READONLY") && !nested_plain {
        return None;
    }
    Some(match (&p.kind, p.array) {
        (_, Shape::Dynamic) => return None,
        (Kind::Bool(_), Shape::Scalar) => FTy::Bool,
        (Kind::Bool(_), Shape::Fixed(n)) => FTy::Arr("bool", n),
        (Kind::Int { .. }, Shape::Scalar) => FTy::I32,
        (Kind::Int { .. }, Shape::Fixed(n)) => FTy::Arr("i32", n),
        (Kind::Float { f64_storage, .. }, Shape::Scalar) => {
            if *f64_storage {
                FTy::F64
            } else {
                FTy::F32
            }
        }
        (Kind::Float { .. }, Shape::Fixed(n)) => FTy::Arr("f32", n),
        (Kind::Float { .. }, Shape::Matrix(r, c)) => FTy::Mat(r, c),
        (Kind::Str { .. }, _) => FTy::Str,
        (Kind::Enum { .. }, _) if p.has_flag("ENUM_FLAG") => FTy::EnumSet,
        (Kind::Enum { .. }, _) => FTy::Enum,
        (Kind::Pointer { is_id: true, .. }, _) => FTy::Id,
        (Kind::Pointer { fixed_type, .. }, _) => {
            FTy::Nested((*plain_by_srna.get(fixed_type.as_str())?).to_owned())
        }
        _ => return None,
    })
}

fn fresh_of<'a>(spec: &'a Spec, srna: &str, prop: &str) -> Option<&'a J> {
    let v = spec.structs.get(srna)?.fresh.as_ref()?.get(prop)?;
    (!v.get("$error").is_some()).then_some(v)
}

fn flat_nums(j: &J) -> Vec<J> {
    let mut out = Vec::new();
    fn go(j: &J, out: &mut Vec<J>) {
        match j {
            J::Array(a) => a.iter().for_each(|x| go(x, out)),
            x => out.push(x.clone()),
        }
    }
    go(j, &mut out);
    out
}

/// The `Default` expression of a field: the fresh value if dumped, else the RNA default.
fn default_expr(f: &Field, fresh: Option<&J>) -> Res<String> {
    let p = &f.prop;
    let ctx = |e: String| format!("{}.{}: {e}", f.srna, p.identifier);
    Ok(match (&f.ty, fresh) {
        (FTy::Nested(n), _) => format!("{n}::default()"),
        (FTy::Id, _) => "None".into(),
        (FTy::Bool, Some(j)) => j
            .as_bool()
            .ok_or_else(|| ctx(format!("bad bool {j}")))?
            .to_string(),
        (FTy::I32, Some(j)) => j
            .as_i64()
            .ok_or_else(|| ctx(format!("bad int {j}")))?
            .to_string(),
        (FTy::F32, Some(j)) => lit_f32(jf64(j).map_err(ctx)?),
        (FTy::F64, Some(j)) => lit_f64(jf64(j).map_err(ctx)?),
        (FTy::Arr(..) | FTy::Mat(..), Some(j)) => {
            let elem = match f.ty {
                FTy::Arr(t, _) => t,
                _ => "f32",
            };
            let lits: Vec<String> = flat_nums(j)
                .iter()
                .map(|x| -> Res<String> {
                    Ok(match elem {
                        "bool" => x.as_bool().ok_or("bad bool")?.to_string(),
                        "i32" => x.as_i64().ok_or("bad int")?.to_string(),
                        _ => lit_f32(jf64(x)?),
                    })
                })
                .collect::<Res<_>>()
                .map_err(ctx)?;
            array_lit(&f.ty, &lits)
        }
        (FTy::Str, Some(j)) => format!("String::from({})", lit_str(j.as_str().unwrap_or(""))),
        (FTy::Enum, Some(j)) => {
            format!("SmolStr::new_static({})", lit_str(j.as_str().unwrap_or("")))
        }
        (FTy::EnumSet, Some(j)) => enum_set_lit(
            &j.as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(str::to_owned))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default(),
        ),
        // RNA defaults.
        (FTy::Bool, None) => match &p.kind {
            Kind::Bool(Def::Scalar(b)) => b.to_string(),
            _ => return Err(ctx("bool default".into())),
        },
        (FTy::I32, None) => match &p.kind {
            Kind::Int {
                default: Def::Scalar(v),
                ..
            } => v.to_string(),
            _ => return Err(ctx("int default".into())),
        },
        (FTy::F32, None) => match &p.kind {
            Kind::Float {
                default: Def::Scalar(v),
                ..
            } => lit_f32(*v),
            _ => return Err(ctx("float default".into())),
        },
        (FTy::F64, None) => match &p.kind {
            Kind::Float {
                default: Def::Scalar(v),
                ..
            } => lit_f64(*v),
            _ => return Err(ctx("float default".into())),
        },
        (FTy::Arr(_, n), None) | (FTy::Mat(n, _), None) => {
            let len = match f.ty {
                FTy::Mat(r, c) => r as usize * c as usize,
                _ => *n as usize,
            };
            let lits: Vec<String> = match &p.kind {
                Kind::Bool(Def::Array(a)) => a.iter().map(|b| b.to_string()).collect(),
                Kind::Bool(Def::Scalar(b)) => vec![b.to_string(); len],
                Kind::Int {
                    default: Def::Array(a),
                    ..
                } => a.iter().map(|v| v.to_string()).collect(),
                Kind::Int {
                    default: Def::Scalar(v),
                    ..
                } => vec![v.to_string(); len],
                Kind::Float {
                    default: Def::Array(a),
                    ..
                } => a.iter().map(|v| lit_f32(*v)).collect(),
                Kind::Float {
                    default: Def::Scalar(v),
                    ..
                } => vec![lit_f32(*v); len],
                _ => return Err(ctx("array default".into())),
            };
            array_lit(&f.ty, &lits)
        }
        (FTy::Str, None) => match &p.kind {
            Kind::Str { default, .. } => format!("String::from({})", lit_str(default)),
            _ => return Err(ctx("string default".into())),
        },
        (FTy::Enum, None) => match &p.kind {
            Kind::Enum {
                default: EnumDef::Item(d),
                ..
            } => format!("SmolStr::new_static({})", lit_str(d)),
            _ => return Err(ctx("enum default".into())),
        },
        (FTy::EnumSet, None) => match &p.kind {
            Kind::Enum {
                default: EnumDef::Flags(f),
                ..
            } => enum_set_lit(f),
            _ => return Err(ctx("enum-flag default".into())),
        },
    })
}

fn array_lit(ty: &FTy, lits: &[String]) -> String {
    match ty {
        FTy::Mat(r, c) => {
            let rows: Vec<String> = (0..*r as usize)
                .map(|i| {
                    format!(
                        "[{}]",
                        lits[i * *c as usize..(i + 1) * *c as usize].join(", ")
                    )
                })
                .collect();
            format!("[{}]", rows.join(", "))
        }
        _ => format!("[{}]", lits.join(", ")),
    }
}

fn enum_set_lit(ids: &[String]) -> String {
    if ids.is_empty() {
        "BTreeSet::new()".into()
    } else {
        format!(
            "BTreeSet::from([{}])",
            ids.iter()
                .map(|i| format!("SmolStr::new_static({})", lit_str(i)))
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

fn get_expr(f: &Field) -> String {
    let n = format!("self.{}", ident(&f.prop.identifier));
    match &f.ty {
        FTy::Bool => format!("Field::Value(Value::Bool({n}))"),
        FTy::I32 => format!("Field::Value(Value::Int({n} as i64))"),
        FTy::F32 => format!("Field::Value(Value::Float({n}))"),
        FTy::F64 => format!("Field::Value(Value::Double({n}))"),
        FTy::Arr("f32", _) => format!(
            "Field::Value(Value::FloatArray {{ data: SmallVec::from_slice(&{n}), shape: prop.array, tag: MathTag::for_subtype(prop.subtype) }})"
        ),
        FTy::Mat(..) => format!(
            "Field::Value(Value::FloatArray {{ data: {n}.iter().flatten().copied().collect(), shape: prop.array, tag: MathTag::for_subtype(prop.subtype) }})"
        ),
        FTy::Arr("i32", _) => {
            format!("Field::Value(Value::IntArray({n}.iter().map(|x| *x as i64).collect()))")
        }
        FTy::Arr(_, _) => format!("Field::Value(Value::BoolArray(SmallVec::from_slice(&{n})))"),
        FTy::Str => format!("Field::Value(Value::Str(SmolStr::from({n}.as_str())))"),
        FTy::Enum => format!("Field::Value(Value::Enum({n}.clone()))"),
        FTy::EnumSet => format!("Field::Value(Value::EnumSet({n}.clone()))"),
        FTy::Id => format!("Field::Id({n})"),
        FTy::Nested(_) => format!("Field::Struct(StructRef::Borrowed(&{n}))"),
    }
}

fn set_stmt(f: &Field) -> Option<String> {
    let n = format!("self.{}", ident(&f.prop.identifier));
    Some(match &f.ty {
        FTy::F64 => format!(
            "{n} = match v {{ Value::Double(d) => d, Value::Float(x) => x as f64, Value::Int(i) => i as f64, _ => {n} }}"
        ),
        FTy::Str => format!("{n} = SmolStr::from_value(&v).to_string()"),
        FTy::Id => format!("{n} = match v {{ Value::Id(x) => x, _ => None }}"),
        FTy::Nested(_) => return None,
        _ => format!("{n} = FromValue::from_value(&v)"),
    })
}

/// Collects the stored fields of a plain struct.
fn fields_of(spec: &Spec, ps: &PlainSpec, plain_by_srna: &BTreeMap<&str, &str>) -> Res<Vec<Field>> {
    let mut out: Vec<Field> = Vec::new();
    let mut seen = BTreeSet::new();
    for srna in ps.srnas {
        let st = spec
            .structs
            .get(*srna)
            .ok_or_else(|| format!("plain struct {}: `{srna}` is not dumped", ps.rust))?;
        let props: Vec<&Prop> = if ps.own_only {
            st.props.iter().collect()
        } else {
            let chain = spec.chain(srna);
            chain
                .iter()
                .rev()
                .filter(|s| s.identifier != "ID")
                .flat_map(|s| s.props.iter())
                .collect()
        };
        for p in props {
            if ps.exclude.contains(&p.identifier.as_str()) {
                continue;
            }
            if let Some(inc) = ps.include
                && !inc.contains(&p.identifier.as_str())
            {
                continue;
            }
            let Some(ty) = field_type(p, plain_by_srna) else {
                continue;
            };
            if !seen.insert(p.identifier.clone()) {
                continue; // union: first defining struct wins
            }
            out.push(Field {
                prop: p.clone(),
                ty,
                srna: (*srna).to_owned(),
            });
        }
    }
    if let Some(inc) = ps.include {
        for i in inc {
            if !seen.contains(*i) {
                return Err(format!(
                    "plain struct {}: included prop `{i}` is not a stored field",
                    ps.rust
                ));
            }
        }
    }
    Ok(out)
}

/// Emits one struct (definition, Default, accessors, optional Rna impl).
#[allow(clippy::too_many_arguments)]
fn emit_struct(
    out: &mut String,
    rust: &str,
    doc: &str,
    srna_static: &str,
    fields: &[Field],
    rna_impl: bool,
    hooked: &[&str],
    extra: &[Extra],
    aliases: &[(&str, &str)],
    fresh: &dyn Fn(&Field) -> Option<J>,
) -> Res<()> {
    // The match pattern of a field: its identifier and those of its aliases.
    let pat = |f: &Field| {
        std::iter::once(f.prop.identifier.as_str())
            .chain(
                aliases
                    .iter()
                    .filter(|(_, t)| *t == f.prop.identifier)
                    .map(|(a, _)| *a),
            )
            .map(lit_str)
            .collect::<Vec<_>>()
            .join(" | ")
    };
    let _ = writeln!(
        out,
        "/// {doc}\n#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]\n#[serde(default)]\npub struct {rust} {{"
    );
    for f in fields {
        let p = &f.prop;
        let mut d = p.description.replace('\n', " ");
        if d.is_empty() {
            d = p.name.clone();
        }
        if matches!(f.ty, FTy::Id) {
            let _ = writeln!(
                out,
                "    /// `{}`: {d} (ID pointer; not serialized, the scene map writes it)\n    #[serde(skip)]",
                p.identifier
            );
        } else {
            let _ = writeln!(out, "    /// `{}`: {d}", p.identifier);
        }
        let _ = writeln!(out, "    pub {}: {},", ident(&p.identifier), f.ty.rust());
    }
    for e in extra {
        let _ = writeln!(
            out,
            "    /// {}\n    pub {}: {},",
            e.doc,
            ident(e.name),
            e.ty.unwrap_or("SmolStr")
        );
    }
    out.push_str("}\n\n");
    let _ = writeln!(
        out,
        "impl Default for {rust} {{\n    fn default() -> Self {{\n        {rust} {{"
    );
    for f in fields {
        let _ = writeln!(
            out,
            "            {}: {},",
            ident(&f.prop.identifier),
            default_expr(f, fresh(f).as_ref())?
        );
    }
    for e in extra {
        if e.ty.is_some() {
            let _ = writeln!(out, "            {}: Default::default(),", ident(e.name));
        } else {
            let _ = writeln!(
                out,
                "            {}: SmolStr::new_static({}),",
                ident(e.name),
                lit_str(e.default)
            );
        }
    }
    out.push_str("        }\n    }\n}\n\n");

    let _ = writeln!(out, "impl {rust} {{");
    let _ = writeln!(
        out,
        "    /// Names of the stored RNA properties, in field order.\n    pub const STORED: [&'static str; {}] = [{}];",
        fields.len(),
        fields
            .iter()
            .map(|f| lit_str(&f.prop.identifier))
            .collect::<Vec<_>>()
            .join(", ")
    );
    // M4 W0: the stored ID pointers by name (user counting and remapping in `IdType`, `.frap`
    // pointers), since serde skips them.
    let ids: Vec<&Field> = fields.iter().filter(|f| matches!(f.ty, FTy::Id)).collect();
    out.push_str("    /// The ID pointers this struct stores, as `(property, value)` (user counting, remapping,\n    /// `.frap`; serde skips them).\n    pub fn id_refs(&self) -> SmallVec<[(&'static str, Option<IdAny>); 2]> {\n");
    if ids.is_empty() {
        out.push_str("        SmallVec::new()\n");
    } else {
        out.push_str("        let mut v = SmallVec::new();\n");
        for f in &ids {
            let _ = writeln!(
                out,
                "        v.push(({}, self.{}));",
                lit_str(&f.prop.identifier),
                ident(&f.prop.identifier)
            );
        }
        out.push_str("        v\n");
    }
    out.push_str("    }\n    /// Sets a stored ID pointer by property name (`false`: not one of `id_refs`).\n    pub fn set_id_ref(&mut self, prop: &str, v: Option<IdAny>) -> bool {\n        match prop {\n");
    for f in &ids {
        let _ = writeln!(
            out,
            "            {} => {{\n                self.{} = v;\n                true\n            }}",
            lit_str(&f.prop.identifier),
            ident(&f.prop.identifier)
        );
    }
    out.push_str("            _ => {\n                let _ = v;\n                false\n            }\n        }\n    }\n");
    out.push_str("    /// Reads a stored property (`None`: not stored in this struct).\n    pub fn rna_get(&self, prop: &'static PropDef) -> Option<Field<'_>> {\n        Some(match prop.identifier {\n");
    for f in fields {
        let get = if hooked.contains(&f.prop.identifier.as_str()) {
            "Field::NeedsHook".to_owned()
        } else {
            get_expr(f)
        };
        let _ = writeln!(out, "            {} => {get},", pat(f));
    }
    out.push_str("            _ => return None,\n        })\n    }\n");
    out.push_str("    /// Writes a stored property; `v` is already coerced against `prop` (§9.2). `None`: not\n    /// stored in this struct (or a nested struct, reached through `rna_struct_mut`).\n    pub fn rna_set(&mut self, prop: &'static PropDef, v: Value) -> Option<Result<SetOutcome, RnaError>> {\n        match prop.identifier {\n");
    for f in fields {
        if hooked.contains(&f.prop.identifier.as_str()) {
            let _ = writeln!(
                out,
                "            {} => return Some(Ok(SetOutcome::NeedsHook)),",
                lit_str(&f.prop.identifier)
            );
            continue;
        }
        if let Some(stmt) = set_stmt(f) {
            let _ = writeln!(out, "            {} => {stmt},", pat(f));
        }
    }
    out.push_str(
        "            _ => return None,\n        }\n        Some(Ok(SetOutcome::Done))\n    }\n",
    );
    out.push_str("    /// Mutable access to a nested plain struct.\n    pub fn rna_struct_mut(&mut self, prop: &'static PropDef) -> Option<RnaMutGuard<'_>> {\n        match prop.identifier {\n");
    for f in fields {
        if matches!(f.ty, FTy::Nested(_)) {
            let _ = writeln!(
                out,
                "            {} => Some(Box::new(&mut self.{})),",
                lit_str(&f.prop.identifier),
                ident(&f.prop.identifier)
            );
        }
    }
    out.push_str("            _ => None,\n        }\n    }\n}\n\n");
    if rna_impl {
        let _ = writeln!(
            out,
            "impl Rna for {rust} {{\n    fn srna(&self) -> &'static StructDef {{\n        &frap_spec_rna::{srna_static}\n    }}\n    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {{\n        match self.rna_get(prop) {{\n            Some(f) => Ok(f),\n            None => unstored_get(self.srna(), prop),\n        }}\n    }}\n}}\n\nimpl RnaMut for {rust} {{\n    fn set(&mut self, prop: &'static PropDef, v: Value) -> Result<SetOutcome, RnaError> {{\n        let srna = self.srna();\n        match self.rna_set(prop, v) {{\n            Some(r) => r,\n            None => unstored_set(srna, prop),\n        }}\n    }}\n    fn struct_mut(&mut self, prop: &'static PropDef) -> Result<RnaMutGuard<'_>, RnaError> {{\n        let srna = self.srna();\n        self.rna_struct_mut(prop).ok_or_else(|| not_a_struct(srna, prop))\n    }}\n    fn coll_mut(&mut self, prop: &'static PropDef) -> Result<RnaCollMutGuard<'_>, RnaError> {{\n        Err(not_a_struct(self.srna(), prop))\n    }}\n}}\n"
        );
    }
    Ok(())
}

const COMMON: &str = r#"
/// `get` of a property this struct does not store: a hook computes it (`Field::NeedsHook`),
/// or it is not a property of the struct at all (`AttributeError`).
pub fn unstored_get(srna: &'static StructDef, prop: &'static PropDef) -> Result<Field<'static>, RnaError> {
    if srna.find_prop(prop.identifier).is_some_and(|p| std::ptr::eq(p, prop)) {
        Ok(Field::NeedsHook)
    } else {
        Err(frap_rna::traits::no_attribute(srna, prop.identifier))
    }
}

/// `set` of a property this struct does not store: read-only props raise Blender's error,
/// others need a global set hook.
pub fn unstored_set(srna: &'static StructDef, prop: &'static PropDef) -> Result<SetOutcome, RnaError> {
    if srna.find_prop(prop.identifier).is_none_or(|p| !std::ptr::eq(p, prop)) {
        Err(frap_rna::traits::no_attribute(srna, prop.identifier))
    } else if prop.is_readonly() {
        Err(frap_rna::traits::read_only(srna, prop.identifier))
    } else {
        Ok(SetOutcome::NeedsHook)
    }
}

/// `struct_mut`/`coll_mut` of a property that is not a stored nested struct.
pub fn not_a_struct(srna: &'static StructDef, prop: &'static PropDef) -> RnaError {
    RnaError::new(
        RnaErrorKind::NotImplemented,
        PyExc::NotImplementedError,
        format!("{}.{} is not a nested struct stored by frap", srna.identifier, prop.identifier),
    )
}
"#;

const USES: &str = "use std::collections::BTreeSet;\n\nuse frap_rna::{Field, FromValue, IdAny, MathTag, PropDef, PyExc, Rna, RnaCollMutGuard, RnaError, RnaErrorKind, RnaMut, RnaMutGuard, SetOutcome, StructDef, StructRef, Value};\nuse serde::{Deserialize, Serialize};\nuse smallvec::SmallVec;\nuse smol_str::SmolStr;\n";

pub fn generate(spec: &Spec, files: &mut Files) -> Res<()> {
    let dir = spec.root.join(DIR);
    let mut plain_by_srna: BTreeMap<&str, &str> = BTreeMap::new();
    for ps in PLAIN {
        if ps.rna_impl {
            plain_by_srna.insert(ps.srnas[0], ps.rust);
        }
    }
    // ---- plain structs
    let mut pm = header(
        "Plain data structs (ARCHITECTURE §4.2): `Default` = Blender 5.2.4 fresh values, serde, RNA accessors.",
    );
    pm.push_str("#![allow(non_snake_case, unused_imports, clippy::all)]\n\n");
    let mut tests = Vec::new();
    for ps in PLAIN {
        let fields = fields_of(spec, ps, &plain_by_srna)?;
        let m = snake(ps.rust);
        let mut s = header(ps.doc.lines().next().unwrap_or(""));
        s.push_str("#![allow(non_snake_case, unused_imports, unused_variables, unreachable_code, clippy::all)]\n");
        s.push_str(USES);
        s.push_str("\nuse super::*;\nuse crate::generated::{not_a_struct, unstored_get, unstored_set};\n\n");
        emit_struct(
            &mut s,
            ps.rust,
            ps.doc,
            &screaming(ps.srnas[0]),
            &fields,
            ps.rna_impl,
            ps.hooked,
            ps.extra,
            &[],
            &|f: &Field| fresh_of(spec, &f.srna, &f.prop.identifier).cloned(),
        )?;
        files.insert(dir.join(format!("plain/{m}.rs")), s.into_bytes());
        let _ = writeln!(pm, "mod {m};\npub use {m}::{};", ps.rust);
        tests.push((
            ps.rust,
            ps.srnas.to_vec(),
            fields
                .iter()
                // Hooked props read through their hook, not `rna_get`.
                .filter(|f| !ps.hooked.contains(&f.prop.identifier.as_str()))
                .map(|f| (f.prop.identifier.clone(), f.srna.clone()))
                .collect::<Vec<_>>(),
        ));
    }
    files.insert(dir.join("plain/mod.rs"), pm.into_bytes());

    // ---- modifier settings
    let mut ms = header(
        "`ModifierType` and `ModifierSettings` (ARCHITECTURE §4.2, §7.1): one settings struct per mesh\n//! modifier type, `Default` = `object.modifiers.new(name, type)` on Blender 5.2.4.",
    );
    ms.push_str("#![allow(non_snake_case, unused_imports, unused_variables, unreachable_code, clippy::all)]\n");
    ms.push_str(USES);
    ms.push_str("\nuse crate::generated::plain::*;\nuse crate::generated::{not_a_struct, unstored_get, unstored_set};\n\n");
    let base_ids: BTreeSet<String> = spec
        .structs
        .get("Modifier")
        .ok_or("Modifier not dumped")?
        .props
        .iter()
        .map(|p| p.identifier.clone())
        .collect();
    let mut variants = Vec::new(); // (variant, struct, type, srna)
    for md in &spec.modifiers {
        let ty = md["type"].as_str().unwrap_or("");
        let srna = md["srna"].as_str().unwrap_or("");
        let st: &Struct = spec
            .structs
            .get(srna)
            .ok_or_else(|| format!("modifier {ty}: struct {srna} not dumped"))?;
        let rust = format!("{}Settings", srna.strip_suffix("Modifier").unwrap_or(srna));
        let aliases: &[(&str, &str)] = MODIFIER_ALIASES
            .iter()
            .find(|(s, _)| *s == srna)
            .map_or(&[], |(_, a)| a);
        let mut fields = Vec::new();
        for p in &st.props {
            if base_ids.contains(&p.identifier) || aliases.iter().any(|(a, _)| *a == p.identifier) {
                continue;
            }
            if let Some(t) = field_type(p, &plain_by_srna) {
                fields.push(Field {
                    prop: p.clone(),
                    ty: t,
                    srna: srna.to_owned(),
                });
            }
        }
        let values = md["values"].clone();
        let extras: &[Extra] = MODIFIER_EXTRAS
            .iter()
            .find(|(s, _)| *s == srna)
            .map_or(&[], |(_, e)| e);
        emit_struct(
            &mut ms,
            &rust,
            &format!("Settings of `{srna}` (`type='{ty}'`)."),
            &screaming(srna),
            &fields,
            true,
            &[],
            extras,
            aliases,
            &|f: &Field| {
                values
                    .get(&f.prop.identifier)
                    .filter(|v| !v.is_object())
                    .cloned()
            },
        )?;
        variants.push((
            camel(ty),
            rust,
            ty.to_owned(),
            srna.to_owned(),
            md["default_name"].clone(),
        ));
    }
    // ModifierType: every Modifier.type item.
    ms.push_str("/// `Modifier.type`: every Blender 5.2 modifier type (grease-pencil types included).\n#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]\npub enum ModifierType {\n");
    for it in &spec.modifier_types {
        let _ = writeln!(
            ms,
            "    /// `{}` ({})\n    {},",
            it.id,
            it.name,
            camel(&it.id)
        );
    }
    ms.push_str("}\n\nimpl ModifierType {\n");
    let _ = writeln!(
        ms,
        "    /// Every type, in `Modifier.type` item order.\n    pub const ALL: [ModifierType; {}] = [{}];",
        spec.modifier_types.len(),
        spec.modifier_types
            .iter()
            .map(|i| format!("ModifierType::{}", camel(&i.id)))
            .collect::<Vec<_>>()
            .join(", ")
    );
    ms.push_str("    /// Blender identifier (`\"SUBSURF\"`).\n    pub const fn identifier(self) -> &'static str {\n        match self {\n");
    for it in &spec.modifier_types {
        let _ = writeln!(
            ms,
            "            ModifierType::{} => {},",
            camel(&it.id),
            lit_str(&it.id)
        );
    }
    ms.push_str("        }\n    }\n    /// Parses a Blender identifier.\n    pub fn from_identifier(s: &str) -> Option<Self> {\n        Some(match s {\n");
    for it in &spec.modifier_types {
        let _ = writeln!(
            ms,
            "            {} => ModifierType::{},",
            lit_str(&it.id),
            camel(&it.id)
        );
    }
    ms.push_str("            _ => return None,\n        })\n    }\n    /// UI name (`\"Subdivision Surface\"`).\n    pub const fn ui_name(self) -> &'static str {\n        match self {\n");
    for it in &spec.modifier_types {
        let _ = writeln!(
            ms,
            "            ModifierType::{} => {},",
            camel(&it.id),
            lit_str(&it.name)
        );
    }
    ms.push_str("        }\n    }\n    /// Name `bpy.ops.object.modifier_add(type=…)` gives a new modifier on a mesh (`None`: not a\n    /// mesh modifier).\n    pub const fn default_name(self) -> Option<&'static str> {\n        match self {\n");
    for (v, _, _, _, name) in &variants {
        if let Some(n) = name.as_str() {
            let _ = writeln!(ms, "            ModifierType::{v} => Some({}),", lit_str(n));
        }
    }
    ms.push_str("            _ => None,\n        }\n    }\n}\n\n");
    ms.push_str("/// `Modifier.type` plus that type's settings (the variants are frozen by M4 W0).\n#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]\n#[non_exhaustive]\npub enum ModifierSettings {\n    /// A modifier type whose settings frap does not store (grease-pencil and volume types, or a\n    /// type unknown to this build): only the type identifier is kept.\n    Untyped {\n        /// `Modifier.type`\n        r#type: SmolStr,\n    },\n");
    for (v, rust, ty, _, _) in &variants {
        let _ = writeln!(ms, "    /// `{ty}`\n    {v}({rust}),");
    }
    ms.push_str("}\n\nimpl ModifierSettings {\n    /// Fresh settings of a type (`modifiers.new` values); `Untyped` for non-mesh types.\n    pub fn new(ty: ModifierType) -> Self {\n        match ty {\n");
    for (v, rust, _, _, _) in &variants {
        let _ = writeln!(
            ms,
            "            ModifierType::{v} => ModifierSettings::{v}({rust}::default()),"
        );
    }
    ms.push_str("            other => ModifierSettings::Untyped { r#type: SmolStr::new_static(other.identifier()) },\n        }\n    }\n    /// `Modifier.type` identifier.\n    pub fn type_identifier(&self) -> &str {\n        match self {\n            ModifierSettings::Untyped { r#type } => r#type.as_str(),\n");
    for (v, _, ty, _, _) in &variants {
        let _ = writeln!(
            ms,
            "            ModifierSettings::{v}(_) => {},",
            lit_str(ty)
        );
    }
    ms.push_str("        }\n    }\n    /// The refined struct (`BevelModifier`; `Modifier` for `Untyped`).\n    pub fn srna(&self) -> &'static StructDef {\n        match self {\n            ModifierSettings::Untyped { .. } => &frap_spec_rna::MODIFIER,\n");
    for (v, _, _, srna, _) in &variants {
        let _ = writeln!(
            ms,
            "            ModifierSettings::{v}(_) => &frap_spec_rna::{},",
            screaming(srna)
        );
    }
    ms.push_str("        }\n    }\n    /// Reads a stored settings property (`None`: not stored).\n    pub fn rna_get(&self, prop: &'static PropDef) -> Option<Field<'_>> {\n        match self {\n            ModifierSettings::Untyped { .. } => None,\n");
    for (v, _, _, _, _) in &variants {
        let _ = writeln!(
            ms,
            "            ModifierSettings::{v}(s) => s.rna_get(prop),"
        );
    }
    ms.push_str("        }\n    }\n    /// Writes a stored settings property (`None`: not stored).\n    pub fn rna_set(&mut self, prop: &'static PropDef, v: Value) -> Option<Result<SetOutcome, RnaError>> {\n        match self {\n            ModifierSettings::Untyped { .. } => None,\n");
    for (v, _, _, _, _) in &variants {
        let _ = writeln!(
            ms,
            "            ModifierSettings::{v}(s) => s.rna_set(prop, v),"
        );
    }
    ms.push_str("        }\n    }\n    /// The ID pointers of the settings (operand objects, textures, node groups), as\n    /// `(property, value)` (M4 W0).\n    pub fn id_refs(&self) -> SmallVec<[(&'static str, Option<IdAny>); 2]> {\n        match self {\n            ModifierSettings::Untyped { .. } => SmallVec::new(),\n");
    for (v, _, _, _, _) in &variants {
        let _ = writeln!(ms, "            ModifierSettings::{v}(s) => s.id_refs(),");
    }
    ms.push_str("        }\n    }\n    /// Sets an ID pointer of the settings by property name (`false`: none of that name).\n    pub fn set_id_ref(&mut self, prop: &str, v: Option<IdAny>) -> bool {\n        match self {\n            ModifierSettings::Untyped { .. } => false,\n");
    for (v, _, _, _, _) in &variants {
        let _ = writeln!(
            ms,
            "            ModifierSettings::{v}(s) => s.set_id_ref(prop, v),"
        );
    }
    ms.push_str("        }\n    }\n}\n");
    files.insert(dir.join("modifier_settings.rs"), ms.into_bytes());

    // ---- root module + generated tests
    let mut root = header(
        "Generated plain structs and modifier settings of `frap-scene` (`cargo xtask codegen`, WP 1.1).",
    );
    root.push_str("#![allow(clippy::all)]\n\nuse frap_rna::{Field, PropDef, PyExc, RnaError, RnaErrorKind, SetOutcome, StructDef};\n\n/// Plain data structs.\npub mod plain;\n/// `ModifierType`, `ModifierSettings` and the per-type settings structs.\npub mod modifier_settings;\n#[cfg(test)]\nmod tests;\n");
    root.push_str(COMMON);
    files.insert(dir.join("mod.rs"), root.into_bytes());

    let mut t = header(
        "Generated tests: every plain struct's `Default` equals the dumped fresh values (test per struct).",
    );
    t.push_str(
        "#![allow(clippy::all)]\nuse super::modifier_settings::*;\nuse super::plain::*;\n\n",
    );
    t.push_str(TEST_HELPERS);
    for (rust, srnas, fields) in &tests {
        let _ = writeln!(
            t,
            "#[test]\nfn default_equals_fresh_{}() {{\n    let v = {rust}::default();",
            snake(rust)
        );
        for (prop, srna) in fields {
            let _ = writeln!(
                t,
                "    check({}, {}, |p| v.rna_get(p));",
                lit_str(srna),
                lit_str(prop)
            );
        }
        let _ = writeln!(t, "    let _ = {:?};\n}}\n", srnas);
    }
    for (v, rust, ty, srna, _) in &variants {
        let _ = writeln!(
            t,
            "#[test]\nfn modifier_default_equals_dump_{}() {{\n    let s = ModifierSettings::new(ModifierType::{v});\n    assert_eq!(s.type_identifier(), {});\n    assert_eq!(s.srna().identifier, {});\n    let ModifierSettings::{v}(inner) = &s else {{ panic!(\"variant\") }};\n    check_modifier({}, |p| inner.rna_get(p), &{rust}::STORED);\n}}\n",
            snake(&camel(ty)),
            lit_str(ty),
            lit_str(srna),
            lit_str(ty)
        );
    }
    files.insert(dir.join("tests.rs"), t.into_bytes());
    Ok(())
}

const TEST_HELPERS: &str = r#"use frap_rna::{Field, PropDef, Value};
use serde_json::Value as J;

fn spec(rel: &str) -> J {
    let p = format!("{}/../../spec/blender-5.2/{rel}", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{p}: {e}"))).unwrap()
}

fn num(j: &J) -> f64 {
    match j {
        J::Number(n) => n.as_f64().unwrap(),
        J::Bool(b) => *b as i64 as f64,
        J::Object(o) => match o["$float"].as_str().unwrap() {
            "inf" => f64::INFINITY,
            "-inf" => f64::NEG_INFINITY,
            _ => f64::NAN,
        },
        _ => panic!("not a number: {j}"),
    }
}

fn flat(j: &J) -> Vec<J> {
    match j {
        J::Array(a) => a.iter().flat_map(flat).collect(),
        x => vec![x.clone()],
    }
}

/// Compares a stored value with the expected JSON value (f32 storage).
fn same(ctx: &str, got: &Value, want: &J) {
    match got {
        Value::Bool(b) => assert_eq!(*b, want.as_bool().unwrap_or(num(want) != 0.0), "{ctx}"),
        Value::Int(i) => assert_eq!(*i, num(want) as i64, "{ctx}"),
        Value::Float(f) => assert_eq!(f.to_bits(), (num(want) as f32).to_bits(), "{ctx}: {f} vs {want}"),
        Value::Double(f) => assert_eq!(*f, num(want), "{ctx}"),
        Value::Str(s) | Value::Enum(s) => assert_eq!(s.as_str(), want.as_str().unwrap(), "{ctx}"),
        Value::EnumSet(s) => {
            let w: Vec<&str> = want.as_array().unwrap().iter().map(|x| x.as_str().unwrap()).collect();
            assert_eq!(s.iter().map(|x| x.as_str()).collect::<Vec<_>>(), w, "{ctx}");
        }
        Value::FloatArray { data, .. } => {
            let w: Vec<u32> = flat(want).iter().map(|x| (num(x) as f32).to_bits()).collect();
            assert_eq!(data.iter().map(|x| x.to_bits()).collect::<Vec<_>>(), w, "{ctx}");
        }
        Value::IntArray(a) => {
            let w: Vec<i64> = flat(want).iter().map(|x| num(x) as i64).collect();
            assert_eq!(a.to_vec(), w, "{ctx}");
        }
        Value::BoolArray(a) => {
            let w: Vec<bool> = flat(want).iter().map(|x| x.as_bool().unwrap()).collect();
            assert_eq!(a.to_vec(), w, "{ctx}");
        }
        other => panic!("{ctx}: unexpected value {other:?}"),
    }
}

/// Expected fresh value of `Struct.prop`: `fresh.differs_from_default`, else the dumped RNA
/// default (for dynamic enums whose dumped default is a placeholder, the overlay default the
/// generator also used is not consulted: such props always have a fresh value).
fn check<'a>(srna: &str, prop: &str, get: impl Fn(&'static PropDef) -> Option<Field<'a>>) {
    let d = spec(&format!("rna/{srna}.json"));
    let ctx = format!("{srna}.{prop}");
    let def = frap_spec_rna::find_struct(srna).unwrap().find_prop(prop).unwrap();
    let want = match d["fresh"]["differs_from_default"].get(prop) {
        Some(v) => v.clone(),
        None => d["props"].as_array().unwrap().iter().find(|p| p["identifier"] == prop).unwrap()["default"].clone(),
    };
    match get(def) {
        Some(Field::Value(v)) => same(&ctx, &v, &want),
        Some(Field::Struct(_)) | Some(Field::Id(None)) => {}
        Some(_) => panic!("{ctx}: unexpected field kind"),
        None => panic!("{ctx}: not stored"),
    }
}

/// Every stored settings field equals `modifiers/<TYPE>.json` `values` (`modifiers.new`).
fn check_modifier<'a>(ty: &str, get: impl Fn(&'static PropDef) -> Option<Field<'a>>, stored: &[&str]) {
    let d = spec(&format!("modifiers/{ty}.json"));
    let srna = frap_spec_rna::find_struct(d["srna"].as_str().unwrap()).unwrap();
    for prop in stored {
        let def = srna.find_prop(prop).unwrap();
        let want = &d["values"][*prop];
        let ctx = format!("{ty}.{prop}");
        match get(def) {
            Some(Field::Value(v)) => same(&ctx, &v, want),
            Some(Field::Struct(_)) => {}
            Some(Field::Id(x)) => assert!(x.is_none() && want.is_null(), "{ctx}"),
            Some(_) => panic!("{ctx}: unexpected field kind"),
            None => panic!("{ctx}: not stored"),
        }
    }
}

"#;
