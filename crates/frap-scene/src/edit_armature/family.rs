// SPDX-License-Identifier: GPL-3.0-or-later
//! The bones of one armature as `_bpy_types.py` `_GenericBone._other_bones` sees them
//! (`armature.bones`, `edit_bones` or `object.pose.bones`, in collection order), for the
//! Python-only bone members (`hooks/bone_python.rs`, `funcs/bone_generic.rs`; M5+M6
//! stabilization).

use frap_rna::traits::ToRnaValue;
use frap_rna::{IdRef, MathTag, ObjectId, PyExc, RnaError, RnaErrorKind, RnaPtr, Seg, Value};
use smol_str::SmolStr;

use super::paths::{
    armature, bone_of, bone_ptr, edit_bone_of, edit_bone_ptr, internal_error, object_armature,
    pose_bone_of, pose_bone_ptr, stale_edit_bone,
};
use crate::{FuncArgs, Host, HostRead, find_hook};

/// Which kind of bone a pointer is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// `Bone`
    Bone,
    /// `EditBone`
    Edit,
    /// `PoseBone`
    Pose,
}

impl Kind {
    /// The kind of `this` (`None`: not a bone).
    pub fn of(this: &RnaPtr) -> Option<Kind> {
        match this.srna.identifier {
            "Bone" => Some(Kind::Bone),
            "EditBone" => Some(Kind::Edit),
            "PoseBone" => Some(Kind::Pose),
            _ => None,
        }
    }
}

/// The bones of `_other_bones`: names and parents (indices into the same list), the bone the
/// pointer names, and how to make a pointer of the same kind.
pub struct Family {
    /// The kind of every bone.
    pub kind: Kind,
    /// Names, in collection order.
    pub names: Vec<SmolStr>,
    /// Parent of each bone (index into `names`).
    pub parents: Vec<Option<usize>>,
    /// The bone `this` names.
    pub index: usize,
    owner: Owner,
}

enum Owner {
    Armature(IdRef<frap_rna::ArmatureId>),
    Object(IdRef<ObjectId>),
}

impl Family {
    /// The canonical pointer of bone `i`.
    pub fn ptr(&self, i: usize) -> RnaPtr {
        let name = &self.names[i];
        match (&self.owner, self.kind) {
            (Owner::Armature(a), Kind::Bone) => bone_ptr(*a, name),
            (Owner::Armature(a), _) => edit_bone_ptr(*a, name),
            (Owner::Object(o), _) => pose_bone_ptr(*o, name),
        }
    }

    /// `parent_index`: the 1-based depth of the bone named `name` among bone `i`'s ancestors
    /// (0: not one of them). Names are compared, as the Python helper does.
    pub fn parent_index(&self, i: usize, name: &str) -> i64 {
        let mut parent = self.parents[i];
        let mut depth = 1;
        while let Some(p) = parent {
            if self.names[p] == name {
                return depth;
            }
            parent = self.parents[p];
            depth += 1;
        }
        0
    }

    /// `children`: the bones whose parent is `i` (for a pose bone, `bone.children` mapped to
    /// the channels; the same order).
    pub fn children(&self, i: usize) -> Vec<usize> {
        (0..self.names.len())
            .filter(|&j| self.parents[j] == Some(i))
            .collect()
    }
}

/// The family of a bone pointer.
pub fn family(h: &dyn HostRead, this: &RnaPtr) -> Result<Family, RnaError> {
    let kind = Kind::of(this).ok_or_else(|| internal_error("not a bone pointer"))?;
    let (names, parents, key, owner): (Vec<SmolStr>, Vec<Option<usize>>, _, _) = match kind {
        Kind::Bone => {
            let (arm, key) = bone_of(this)?;
            let a = armature(h.main(), arm)?;
            (
                a.bones.iter().map(|b| b.name.clone()).collect(),
                a.bones.iter().map(|b| b.parent).collect(),
                key,
                Owner::Armature(arm),
            )
        }
        Kind::Edit => {
            let (arm, key) = edit_bone_of(this)?;
            let ea = h
                .edit()
                .armatures
                .get(&arm.key())
                .ok_or_else(stale_edit_bone)?;
            (
                ea.edit_bones.iter().map(|b| b.name.clone()).collect(),
                ea.edit_bones.iter().map(|b| b.parent).collect(),
                key,
                Owner::Armature(arm),
            )
        }
        Kind::Pose => {
            let (ob, key) = pose_bone_of(this)?;
            let object = h
                .main()
                .objects
                .get(ob)
                .ok_or_else(|| internal_error("the object was removed"))?;
            let arm = object_armature(h.main(), ob)
                .ok_or_else(|| internal_error("not an armature object"))?;
            let a = armature(h.main(), arm)?;
            let pose = object
                .pose
                .as_ref()
                .ok_or_else(|| internal_error("the pose was removed"))?;
            let names: Vec<SmolStr> = pose.bones.iter().map(|b| b.name.clone()).collect();
            let parents = pose
                .bones
                .iter()
                .map(|c| {
                    let bone = a.bone_index(&c.name)?;
                    let p = a.bones[bone].parent?;
                    names.iter().position(|n| *n == a.bones[p].name)
                })
                .collect();
            (names, parents, key, Owner::Object(ob))
        }
    };
    let index = key
        .find(names.iter().map(|n| n.as_str()))
        .ok_or_else(|| internal_error("the bone was removed"))?;
    Ok(Family {
        kind,
        names,
        parents,
        index,
        owner,
    })
}

/// `basename`: the name before the first `.`.
pub fn basename(name: &str) -> &str {
    name.split('.').next().unwrap_or("")
}

/// `_GenericBone.translate(vec)` (blender/scripts/modules/_bpy_types.py): `self.head += vec;
/// self.tail += vec`. An edit bone runs its `head` and `tail` setters (connected parents and
/// children, the X mirror follow); `Bone.head` and `PoseBone.head` are read-only, so they raise
/// `bpy_struct: attribute "head" from "Bone" is read-only` (verified on 5.2.4 with a `Vector`
/// argument).
pub fn translate(h: &mut dyn Host, this: &RnaPtr, args: &mut FuncArgs) -> Result<Value, RnaError> {
    if Kind::of(this) != Some(Kind::Edit) {
        return Err(RnaError::new(
            RnaErrorKind::ReadOnly,
            PyExc::AttributeError,
            format!(
                "bpy_struct: attribute \"head\" from \"{}\" is read-only",
                this.srna.identifier
            ),
        ));
    }
    let vec = match args.get("vec") {
        Some(Value::FloatArray { data, .. }) if data.len() == 3 => [data[0], data[1], data[2]],
        _ => return Err(internal_error("translate(): vec is not a 3D vector")),
    };
    for name in ["head", "tail"] {
        let prop = this
            .srna
            .find_prop(name)
            .ok_or_else(|| internal_error("EditBone without head/tail"))?;
        let cur = {
            let (arm, key) = crate::edit_armature::paths::edit_bone_of(this)?;
            let ea = h
                .edit()
                .armatures
                .get(&arm.key())
                .ok_or_else(crate::edit_armature::paths::stale_edit_bone)?;
            let i = key
                .find(ea.edit_bones.iter().map(|b| b.name.as_str()))
                .ok_or_else(crate::edit_armature::paths::stale_edit_bone)?;
            if name == "head" {
                ea.edit_bones[i].head
            } else {
                ea.edit_bones[i].tail
            }
        };
        let new = [cur[0] + vec[0], cur[1] + vec[1], cur[2] + vec[2]];
        let set = find_hook(this.srna, name)
            .and_then(|r| r.set)
            .ok_or_else(|| internal_error("EditBone head/tail without a set hook"))?;
        set(h, this, prop, new.to_rna_value(prop, MathTag::Vector))?;
    }
    Ok(Value::None)
}

/// `_GenericBone.parent_index(parent_test)`: names are compared along this bone's parents (any
/// bone type may be passed; `None` raises `'NoneType' object has no attribute 'name'`).
pub fn parent_index(
    h: &mut dyn Host,
    this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    let name = match args.get("parent_test") {
        Some(Value::Ptr(p)) => match p.path.last() {
            Some(Seg::Key(k)) if Kind::of(p).is_some() => k.clone(),
            _ => {
                return Err(RnaError::new(
                    RnaErrorKind::NoAttribute,
                    PyExc::AttributeError,
                    format!("'{}' object has no attribute 'name'", p.srna.identifier),
                ));
            }
        },
        _ => {
            return Err(RnaError::new(
                RnaErrorKind::NoAttribute,
                PyExc::AttributeError,
                "'NoneType' object has no attribute 'name'",
            ));
        }
    };
    let f = family(h, this)?;
    Ok(Value::Int(f.parent_index(f.index, &name)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basename_is_before_the_first_dot() {
        assert_eq!(basename("arm.L.001"), "arm");
        assert_eq!(basename("Bone"), "Bone");
        assert_eq!(basename(".x"), "");
    }
}
