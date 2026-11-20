// SPDX-License-Identifier: GPL-3.0-or-later
//! The Python-only bone members of `_bpy_types.py` (`_GenericBone`, `PoseBone.children`,
//! `EditBone.children`; M5+M6 stabilization), on `Bone`, `EditBone` and `PoseBone`:
//! `vector`, `center`, `x_axis`/`y_axis`/`z_axis`, `basename`, `parent_recursive`,
//! `children`, `children_recursive`, `children_recursive_basename`, and the functions
//! `translate(vec)` and `parent_index(parent_test)` (`funcs/bone_generic.rs`).
//!
//! Ported from blender/scripts/modules/_bpy_types.py (Blender 5.2.4, GPL-2.0-or-later): each
//! member reads the bone's own RNA values (`head`, `tail`, `matrix`, `parent`, `name`) as the
//! Python property does, so `Bone.vector` is in the parent's space (`Bone.head`/`tail`),
//! `EditBone.vector` in armature space and `PoseBone.vector` in pose space; `x_axis` is the
//! first column of `matrix.to_3x3()`. `_other_bones` is `armature.bones`, `edit_bones` or
//! `object.pose.bones`, in their collection order; `children_recursive` sorts by depth
//! (stable). Collections answer tuples of the bones' canonical pointers.

use frap_rna::{Field, MathTag, PropDef, PyExc, RnaError, RnaErrorKind, RnaPtr, Value, rna_hook};
use glam::{Mat3, Vec3};

use crate::edit_armature::family::{Family, Kind, basename, family};
use crate::edit_armature::paths::{edit_bone_of, internal_error, stale_edit_bone};
use crate::{HostRead, find_hook};

/// A float vector or matrix RNA value as floats.
fn floats(f: Field<'_>) -> Result<Vec<f32>, RnaError> {
    match f {
        Field::Value(Value::FloatArray { data, .. }) => Ok(data.to_vec()),
        _ => Err(internal_error("expected a float array")),
    }
}

/// Reads RNA property `name` of the bone `this` through its get hook, or the edit bone's
/// stored value.
fn read(h: &dyn HostRead, this: &RnaPtr, name: &str) -> Result<Vec<f32>, RnaError> {
    if Kind::of(this) == Some(Kind::Edit) {
        let (arm, key) = edit_bone_of(this)?;
        let ea = h
            .edit()
            .armatures
            .get(&arm.key())
            .ok_or_else(stale_edit_bone)?;
        let i = key
            .find(ea.edit_bones.iter().map(|b| b.name.as_str()))
            .ok_or_else(stale_edit_bone)?;
        let b = &ea.edit_bones[i];
        return Ok(match name {
            "head" => b.head.to_vec(),
            "tail" => b.tail.to_vec(),
            _ => ea.bone_mat4(i).transpose().to_cols_array().to_vec(),
        });
    }
    let prop = this
        .srna
        .find_prop(name)
        .ok_or_else(|| internal_error(format!("{} has no {name}", this.srna.identifier)))?;
    let get = find_hook(this.srna, name)
        .and_then(|r| r.get)
        .ok_or_else(|| {
            internal_error(format!("{}.{name} has no get hook", this.srna.identifier))
        })?;
    floats(get(h, this, prop)?)
}

fn vec3(v: &[f32]) -> Vec3 {
    Vec3::new(v[0], v[1], v[2])
}

/// `matrix.to_3x3()` of a row-major 3x3 or 4x4 RNA matrix.
fn mat3(v: &[f32]) -> Mat3 {
    let n = if v.len() == 16 { 4 } else { 3 };
    let at = |r: usize, c: usize| v[r * n + c];
    Mat3::from_cols(
        Vec3::new(at(0, 0), at(1, 0), at(2, 0)),
        Vec3::new(at(0, 1), at(1, 1), at(2, 1)),
        Vec3::new(at(0, 2), at(1, 2), at(2, 2)),
    )
}

fn vector_value(prop: &PropDef, v: Vec3) -> Value {
    use frap_rna::traits::ToRnaValue;
    v.to_array().to_rna_value(prop, MathTag::Vector)
}

fn tuple(f: &Family, items: impl IntoIterator<Item = usize>) -> Value {
    Value::Tuple(
        items
            .into_iter()
            .map(|i| Value::Ptr(Box::new(f.ptr(i))))
            .collect(),
    )
}

/// The value of Python-only member `prop` of bone `this`.
pub fn member(h: &dyn HostRead, this: &RnaPtr, prop: &PropDef) -> Result<Value, RnaError> {
    Ok(match prop.identifier {
        "vector" | "center" => {
            let (head, tail) = (vec3(&read(h, this, "head")?), vec3(&read(h, this, "tail")?));
            let v = if prop.identifier == "vector" {
                tail - head
            } else {
                (head + tail) * 0.5
            };
            vector_value(prop, v)
        }
        "x_axis" | "y_axis" | "z_axis" => {
            let m = mat3(&read(h, this, "matrix")?);
            let col = match prop.identifier {
                "x_axis" => m.x_axis,
                "y_axis" => m.y_axis,
                _ => m.z_axis,
            };
            vector_value(prop, col)
        }
        "basename" => {
            let f = family(h, this)?;
            Value::Str(basename(&f.names[f.index]).into())
        }
        "parent_recursive" => {
            let f = family(h, this)?;
            let mut out = Vec::new();
            let mut p = f.parents[f.index];
            while let Some(i) = p {
                out.push(i);
                p = f.parents[i];
            }
            tuple(&f, out)
        }
        "children" => {
            let f = family(h, this)?;
            let c = f.children(f.index);
            tuple(&f, c)
        }
        "children_recursive" => {
            let f = family(h, this)?;
            let name = f.names[f.index].clone();
            let mut found: Vec<(i64, usize)> = (0..f.names.len())
                .filter_map(|j| {
                    let d = f.parent_index(j, &name);
                    (d != 0).then_some((d, j))
                })
                .collect();
            found.sort_by_key(|(d, _)| *d);
            tuple(&f, found.into_iter().map(|(_, j)| j))
        }
        "children_recursive_basename" => {
            let f = family(h, this)?;
            let base = basename(&f.names[f.index]).to_owned();
            let mut chain = Vec::new();
            let mut child = f.index;
            loop {
                let same: Vec<usize> = f
                    .children(child)
                    .into_iter()
                    .filter(|&c| basename(&f.names[c]) == base)
                    .collect();
                if same.len() == 1 {
                    child = same[0];
                    chain.push(child);
                } else {
                    break;
                }
            }
            tuple(&f, chain)
        }
        other => {
            return Err(RnaError::new(
                RnaErrorKind::NoAttribute,
                PyExc::AttributeError,
                format!(
                    "'{}' object has no attribute '{other}'",
                    this.srna.identifier
                ),
            ));
        }
    })
}

macro_rules! members {
    ($($srna:literal => [$($f:ident = $p:literal),* $(,)?]),* $(,)?) => {
        $($(
            #[rna_hook(srna = $srna, prop = $p, krate = crate)]
            fn $f<'h>(
                h: &'h dyn HostRead,
                this: &RnaPtr,
                prop: &'static PropDef,
            ) -> Result<Field<'h>, RnaError> {
                member(h, this, prop).map(Field::Value)
            }

            // The bone bindings declare no Python-only member (`rna_struct!`), so each
            // hook declares its own binding here, as `View3DShading.studio_light` does.
            const _: () = {
                #[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
                #[frap_rna::linkme::distributed_slice(frap_rna::testing::HOOK_DECLS)]
                #[linkme(crate = frap_rna::linkme)]
                static DECL: frap_rna::testing::HookDecl = frap_rna::testing::HookDecl {
                    srna: $srna,
                    prop: $p,
                    coll: false,
                };
            };
        )*)*
    };
}

members! {
    "Bone" => [
        bone_vector = "vector",
        bone_center = "center",
        bone_x_axis = "x_axis",
        bone_y_axis = "y_axis",
        bone_z_axis = "z_axis",
        bone_basename = "basename",
        bone_parent_recursive = "parent_recursive",
        bone_children_recursive = "children_recursive",
        bone_children_recursive_basename = "children_recursive_basename",
    ],
    "EditBone" => [
        edit_vector = "vector",
        edit_center = "center",
        edit_x_axis = "x_axis",
        edit_y_axis = "y_axis",
        edit_z_axis = "z_axis",
        edit_basename = "basename",
        edit_parent_recursive = "parent_recursive",
        edit_children = "children",
        edit_children_recursive = "children_recursive",
        edit_children_recursive_basename = "children_recursive_basename",
    ],
    "PoseBone" => [
        pose_vector = "vector",
        pose_center = "center",
        pose_x_axis = "x_axis",
        pose_y_axis = "y_axis",
        pose_z_axis = "z_axis",
        pose_basename = "basename",
        pose_parent_recursive = "parent_recursive",
        pose_children = "children",
        pose_children_recursive = "children_recursive",
        pose_children_recursive_basename = "children_recursive_basename",
    ],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matrix_columns() {
        let m = mat3(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0]);
        assert_eq!(m.x_axis, Vec3::new(1.0, 4.0, 7.0));
        let m4 = mat3(&[
            1.0, 2.0, 3.0, 0.0, 4.0, 5.0, 6.0, 0.0, 7.0, 8.0, 9.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ]);
        assert_eq!(m4.z_axis, Vec3::new(3.0, 6.0, 9.0));
    }
}
