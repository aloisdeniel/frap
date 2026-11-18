// SPDX-License-Identifier: GPL-3.0-or-later
//! Blender-RNA-shaped property entries (§14 `properties[]`) and the value encodings shared by
//! the manifest, the JSON Schemas and the tool definitions.

use frap_op::{OperatorDef, PollReq};
use frap_rna::{
    ArrayShape, Defaults, EnumDefault, FloatStorage, OperatorSpec, PropDef, PropFlags, PropKind,
    Unit,
};
use serde_json::{Map, Value as J, json};

use super::curated;

/// Where a property lives, which decides the frap-specific keys of its entry.
#[derive(Copy, Clone)]
pub enum PropOwner<'a> {
    /// An operator property (`unset_semantics`, `frap_note`, `frap_extension`, `context_rule`).
    Op {
        /// The operator spec.
        spec: &'static OperatorSpec,
        /// Its implementation, if registered.
        def: Option<&'a OperatorDef>,
    },
    /// An RNA struct property (`readonly`, `python_only`).
    Struct,
    /// An RNA function parameter or return value (`optional`).
    Param,
}

/// Blender's unit string, with angles spelled `ROTATION (radians)` (§14, decision log #31).
pub fn unit_label(unit: Unit) -> &'static str {
    match unit {
        Unit::Rotation => "ROTATION (radians)",
        u => u.as_str(),
    }
}

/// Protocol name of a declarative poll requirement (`MODE_EDIT_MESH`, `ACTIVE_TYPE(MESH)`), as in
/// the §13.5 `detail.requires` example.
pub fn poll_req_name(r: &PollReq) -> String {
    match r {
        PollReq::ModeObject => "MODE_OBJECT".into(),
        PollReq::ModeEditMesh => "MODE_EDIT_MESH".into(),
        PollReq::ModeEditArmature => "MODE_EDIT_ARMATURE".into(),
        PollReq::ModePose => "MODE_POSE".into(),
        PollReq::ActiveObject => "ACTIVE_OBJECT".into(),
        PollReq::ActiveType(t) => format!("ACTIVE_TYPE({t})"),
        PollReq::ActiveEditable => "ACTIVE_EDITABLE".into(),
        PollReq::HasSelectedObjects => "HAS_SELECTED_OBJECTS".into(),
        PollReq::SceneHasCamera => "SCENE_HAS_CAMERA".into(),
        PollReq::SceneEditable => "SCENE_EDITABLE".into(),
        PollReq::ContextImage => "CONTEXT_IMAGE".into(),
        // `PollReq` is non-exhaustive: later variants get their Debug name in SCREAMING_CASE.
        other => screaming(&format!("{other:?}")),
    }
}

/// Human text of a declarative poll requirement.
pub fn poll_req_text(r: &PollReq) -> String {
    match r {
        PollReq::ModeObject => "Object Mode (bpy.ops.object.mode_set(mode='OBJECT'))".into(),
        PollReq::ModeEditMesh => {
            "Edit Mode on a mesh (bpy.ops.object.mode_set(mode='EDIT') with a mesh active)".into()
        }
        PollReq::ModeEditArmature => "Edit Mode on an armature".into(),
        PollReq::ModePose => "Pose Mode (bpy.ops.object.mode_set(mode='POSE'))".into(),
        PollReq::ActiveObject => "an active object (bpy.context.view_layer.objects.active)".into(),
        PollReq::ActiveType(t) => format!("the active object is of type {t}"),
        PollReq::ActiveEditable => "the active object is editable (not linked)".into(),
        PollReq::HasSelectedObjects => "at least one selected object".into(),
        PollReq::SceneHasCamera => "scene.camera is set".into(),
        PollReq::SceneEditable => "an editable scene".into(),
        PollReq::ContextImage => {
            "the Image Editor's image (bpy.context.edit_image), which headless Blender and frap \
             never have"
                .into()
        }
        other => format!("{other:?}"),
    }
}

fn screaming(debug: &str) -> String {
    let mut s = String::new();
    for (i, c) in debug.chars().enumerate() {
        if !c.is_ascii_alphanumeric() {
            break;
        }
        if c.is_ascii_uppercase() && i > 0 {
            s.push('_');
        }
        s.push(c.to_ascii_uppercase());
    }
    s
}

/// A float as JSON: f32-stored values in their shortest round-trip form (§13.3), integral values
/// as integers, non-finite values tagged `{"$float": "inf"}`.
pub fn float_json(x: f64, storage: FloatStorage) -> J {
    if x.is_nan() {
        return json!({"$float": "nan"});
    }
    if x.is_infinite() {
        return json!({"$float": if x > 0.0 { "inf" } else { "-inf" }});
    }
    let v = match storage {
        FloatStorage::F32 => frap_math::fmt::shortest_f64(x as f32),
        FloatStorage::F64 => x,
    };
    if v.fract() == 0.0 && v.abs() < 9.007_199e15 {
        return J::from(v as i64);
    }
    serde_json::Number::from_f64(v).map_or(J::Null, J::Number)
}

fn storage(p: &PropDef) -> FloatStorage {
    match p.kind {
        PropKind::Float { storage, .. } => storage,
        _ => FloatStorage::F64,
    }
}

fn matrix_rows(flat: Vec<J>, shape: ArrayShape) -> J {
    match shape {
        ArrayShape::Matrix(_, c) if c > 0 => J::Array(
            flat.chunks(c as usize)
                .map(|row| J::Array(row.to_vec()))
                .collect(),
        ),
        _ => J::Array(flat),
    }
}

fn defaults_json<T: Copy>(d: &Defaults<T>, shape: ArrayShape, conv: impl Fn(T) -> J) -> J {
    match d {
        Defaults::Scalar(x) => match shape {
            // Blender reports one scalar default for every element of an array prop.
            ArrayShape::Fixed(n) => J::Array(vec![conv(*x); n as usize]),
            ArrayShape::Matrix(r, c) => matrix_rows(vec![conv(*x); r as usize * c as usize], shape),
            _ => conv(*x),
        },
        Defaults::Array(a) => matrix_rows(a.iter().map(|x| conv(*x)).collect(), shape),
    }
}

/// The default value of a property, in the response value encoding (§13.3): matrices as rows,
/// enum flags as sorted arrays, pointers `null`, collections `[]`.
pub fn default_json(p: &PropDef) -> J {
    match &p.kind {
        PropKind::Bool { default } => defaults_json(default, p.array, J::from),
        PropKind::Int { default, .. } => defaults_json(default, p.array, J::from),
        PropKind::Float {
            default, storage, ..
        } => defaults_json(default, p.array, |x| float_json(x, *storage)),
        PropKind::String { default, .. } => J::from(*default),
        PropKind::Enum { default, .. } => match default {
            EnumDefault::Item(s) => J::from(*s),
            EnumDefault::Flags(f) => {
                let mut v: Vec<&str> = f.to_vec();
                v.sort_unstable();
                J::from(v)
            }
        },
        PropKind::Pointer { .. } => J::Null,
        PropKind::Collection { .. } => J::Array(Vec::new()),
        _ => J::Null,
    }
}

/// `null` | `[n]` | `[rows, cols]` | `"dynamic"`.
pub fn array_json(shape: ArrayShape) -> J {
    match shape {
        ArrayShape::Scalar => J::Null,
        ArrayShape::Fixed(n) => json!([n]),
        ArrayShape::Matrix(r, c) => json!([r, c]),
        ArrayShape::Dynamic => J::from("dynamic"),
    }
}

/// Hard and soft ranges of a numeric property, `None` for other types.
pub fn ranges(p: &PropDef) -> Option<[J; 4]> {
    match &p.kind {
        PropKind::Int { hard, soft, .. } => Some([
            J::from(hard.0),
            J::from(hard.1),
            J::from(soft.0),
            J::from(soft.1),
        ]),
        PropKind::Float { hard, soft, .. } => {
            let s = storage(p);
            Some([
                float_json(hard.0, s),
                float_json(hard.1, s),
                float_json(soft.0, s),
                float_json(soft.1, s),
            ])
        }
        _ => None,
    }
}

/// The note frap attaches to a dynamic enum.
pub fn context_rule(p: &PropDef, def: Option<&OperatorDef>) -> Option<String> {
    let PropKind::Enum {
        dynamic: Some(key), ..
    } = &p.kind
    else {
        return None;
    };
    let narrowed = def.is_some_and(|d| d.dyn_enums.iter().any(|(n, _)| *n == p.identifier));
    Some(if narrowed {
        format!(
            "items depend on the context in Blender ({key}); frap narrows them to the live context, enum_items is the superset"
        )
    } else {
        format!(
            "items come from a Blender callback that depends on the context ({key}); enum_items is frap's verified superset (spec/overlay/dynamic_enums.toml)"
        )
    })
}

/// One `properties[]` entry (§14).
pub fn describe_prop(p: &PropDef, owner: PropOwner<'_>) -> J {
    let mut m = Map::new();
    m.insert("identifier".into(), J::from(p.identifier));
    m.insert("name".into(), J::from(p.name));
    m.insert("description".into(), J::from(p.description));
    m.insert("type".into(), J::from(p.type_str()));
    m.insert("subtype".into(), J::from(p.subtype.as_str()));
    m.insert("unit".into(), J::from(unit_label(p.unit)));
    m.insert("array".into(), array_json(p.array));
    m.insert("default".into(), default_json(p));
    if let Some([hmin, hmax, smin, smax]) = ranges(p) {
        m.insert("hard_min".into(), hmin);
        m.insert("hard_max".into(), hmax);
        m.insert("soft_min".into(), smin);
        m.insert("soft_max".into(), smax);
    }
    match &p.kind {
        PropKind::Float { precision, .. } => {
            m.insert("precision".into(), J::from(*precision));
        }
        PropKind::String { max_bytes, .. } => {
            m.insert("length_max".into(), J::from(*max_bytes));
        }
        PropKind::Enum { items, dynamic, .. } => {
            m.insert(
                "enum_items".into(),
                J::Array(
                    items
                        .iter()
                        .map(|it| {
                            json!({
                                "identifier": it.identifier,
                                "name": it.name,
                                "description": it.description,
                                "value": it.value,
                            })
                        })
                        .collect(),
                ),
            );
            m.insert(
                "is_enum_flag".into(),
                J::from(p.flags.contains(PropFlags::ENUM_FLAG)),
            );
            m.insert("dynamic_enum".into(), J::from(*dynamic));
            let def = match owner {
                PropOwner::Op { def, .. } => def,
                _ => None,
            };
            m.insert("context_rule".into(), J::from(context_rule(p, def)));
        }
        PropKind::Pointer { fixed_type, is_id } => {
            m.insert("fixed_type".into(), J::from(*fixed_type));
            m.insert("is_id".into(), J::from(*is_id));
        }
        PropKind::Collection { item_type, api } => {
            m.insert("item_type".into(), J::from(*item_type));
            m.insert("api".into(), J::from(*api));
        }
        _ => {}
    }
    m.insert(
        "hidden".into(),
        J::from(p.flags.contains(PropFlags::HIDDEN)),
    );
    m.insert(
        "skip_save".into(),
        J::from(p.flags.contains(PropFlags::SKIP_SAVE)),
    );
    m.insert("flags".into(), J::from(p.flags.names()));
    if let Some(d) = p.deprecated {
        m.insert(
            "deprecated".into(),
            json!({"note": d.note, "version": d.version, "removal_version": d.removal_version}),
        );
    }
    match owner {
        PropOwner::Op { spec, .. } => {
            m.insert(
                "required".into(),
                J::from(p.flags.contains(PropFlags::REQUIRED)),
            );
            m.insert(
                "unset_semantics".into(),
                J::from(curated::unset_semantics(spec, p.identifier)),
            );
            m.insert("frap_note".into(), J::from(frap_note(spec, p)));
            m.insert(
                "frap_extension".into(),
                J::from(p.identifier.starts_with("frap_") || spec.idname.starts_with("frap.")),
            );
        }
        PropOwner::Struct => {
            m.insert("readonly".into(), J::from(p.is_readonly()));
            m.insert(
                "python_only".into(),
                J::from(p.flags.contains(PropFlags::PYTHON_ONLY)),
            );
        }
        PropOwner::Param => {
            m.insert(
                "optional".into(),
                J::from(p.flags.contains(PropFlags::ARGUMENT_OPTIONAL)),
            );
        }
    }
    J::Object(m)
}

/// frap's note on an operator property: where frap's handling is worth knowing.
fn frap_note(spec: &OperatorSpec, p: &PropDef) -> Option<String> {
    if p.unit == Unit::Rotation {
        return Some(
            "radians; a whole number above 2π gets the ANGLE_LIKELY_DEGREES hint (use radians(90))"
                .into(),
        );
    }
    if let Some(s) = curated::DEFAULTS_THAT_SURPRISE.iter().find(|s| {
        s.kind == curated::SurpriseKind::OperatorProperty
            && s.target
                .strip_prefix(spec.idname)
                .and_then(|r| r.strip_prefix('.'))
                == Some(p.identifier)
    }) {
        return Some(s.why.into());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floats() {
        assert_eq!(
            float_json(0.10000000149011612, FloatStorage::F32),
            json!(0.1)
        );
        assert_eq!(float_json(2.0, FloatStorage::F32), json!(2));
        assert_eq!(
            float_json(999999995904.0, FloatStorage::F32),
            json!(1000000000000_i64)
        );
        assert_eq!(
            float_json(f64::INFINITY, FloatStorage::F32),
            json!({"$float": "inf"})
        );
        assert_eq!(float_json(0.1, FloatStorage::F64), json!(0.1));
    }

    #[test]
    fn poll_names() {
        assert_eq!(poll_req_name(&PollReq::ModeEditMesh), "MODE_EDIT_MESH");
        assert_eq!(
            poll_req_name(&PollReq::ActiveType("MESH")),
            "ACTIVE_TYPE(MESH)"
        );
        assert_eq!(screaming("HasSelectedObjects"), "HAS_SELECTED_OBJECTS");
    }

    #[test]
    fn units() {
        assert_eq!(unit_label(Unit::Rotation), "ROTATION (radians)");
        assert_eq!(unit_label(Unit::Length), "LENGTH");
    }
}
