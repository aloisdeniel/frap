// SPDX-License-Identifier: GPL-3.0-or-later
//! What the NLA needs to know about an animated property without the data
//! (`nlaevalchan_verify` → `RNA_path_resolve_property`, `nlaevalchan_detect_mix_mode`,
//! `nlaevalchan_get_default_values` in blender/source/blender/blenkernel/intern/anim_sys.cc,
//! Blender 5.2.4): the F-curve's `data_path` is walked through the generated RNA definitions
//! from the animated ID's struct, giving the array length, the mix mode (quaternion, axis-angle,
//! multiplicative, additive) and the RNA default values the NLA blends from.
//!
//! A path the definitions cannot follow (a refined item type such as
//! `modifiers["Bevel"].width`, whose item struct is the `Modifier` base) falls back to one
//! scalar channel per array index, additive, with default 0.

use frap_rna::{
    ArrayShape, Defaults, EnumDefault, PropDef, PropFlags, PropKind, StructDef, Subtype,
};
use smol_str::SmolStr;

/// `NEC_MIX_*`
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum MixMode {
    /// `NEC_MIX_ADD`
    Add,
    /// `NEC_MIX_MULTIPLY` (`PROP_PROPORTIONAL` properties: scales, resolutions)
    Multiply,
    /// `NEC_MIX_QUATERNION`
    Quaternion,
    /// `NEC_MIX_AXIS_ANGLE`
    AxisAngle,
}

/// The resolved property of one NLA channel.
#[derive(Clone, Debug, PartialEq)]
pub struct ChannelProp {
    /// `RNA_property_array_check`
    pub is_array: bool,
    /// Number of components (1 for scalars).
    pub length: usize,
    /// Mix mode.
    pub mix_mode: MixMode,
    /// `nlaevalchan_get_default_values`
    pub defaults: Vec<f32>,
    /// `RNA_property_animateable` (from the definition's `ANIMATABLE` flag).
    pub animatable: bool,
}

/// One segment of an RNA path.
#[derive(Clone, Debug, PartialEq)]
pub enum PathSeg {
    /// `.name`
    Prop(SmolStr),
    /// `[3]`
    Index(i64),
    /// `["key"]`
    Key(SmolStr),
}

/// Parses an RNA path (`pose.bones["Bone"].location`, `key_blocks['Key 1'].value`,
/// `nodes["A"].inputs[0].default_value`). `None` when it is malformed.
pub fn parse_path(path: &str) -> Option<Vec<PathSeg>> {
    let mut out = Vec::new();
    let b = path.as_bytes();
    let mut i = 0;
    let mut expect_ident = true;
    while i < b.len() {
        match b[i] {
            b'.' => {
                if expect_ident {
                    return None;
                }
                expect_ident = true;
                i += 1;
            }
            b'[' => {
                if expect_ident {
                    return None;
                }
                i += 1;
                if i < b.len() && (b[i] == b'"' || b[i] == b'\'') {
                    let q = b[i];
                    i += 1;
                    let mut key = String::new();
                    while i < b.len() && b[i] != q {
                        if b[i] == b'\\' && i + 1 < b.len() {
                            i += 1;
                        }
                        let start = i;
                        // Copy one UTF-8 character.
                        i += 1;
                        while i < b.len() && (b[i] & 0xC0) == 0x80 {
                            i += 1;
                        }
                        key.push_str(std::str::from_utf8(&b[start..i]).ok()?);
                    }
                    if i >= b.len() {
                        return None;
                    }
                    i += 1;
                    out.push(PathSeg::Key(key.into()));
                } else {
                    let start = i;
                    while i < b.len() && (b[i] == b'-' || b[i].is_ascii_digit()) {
                        i += 1;
                    }
                    out.push(PathSeg::Index(path.get(start..i)?.parse().ok()?));
                }
                if i >= b.len() || b[i] != b']' {
                    return None;
                }
                i += 1;
            }
            c if c == b'_' || c.is_ascii_alphabetic() => {
                if !expect_ident {
                    return None;
                }
                let start = i;
                while i < b.len() && (b[i] == b'_' || b[i].is_ascii_alphanumeric()) {
                    i += 1;
                }
                out.push(PathSeg::Prop(path[start..i].into()));
                expect_ident = false;
            }
            _ => return None,
        }
    }
    if expect_ident || out.is_empty() {
        return None;
    }
    Some(out)
}

/// The struct an animated ID of type `id_type` (an `ActionSlot.target_id_type` identifier)
/// starts paths from.
pub fn root_struct(id_type: &str) -> Option<&'static StructDef> {
    let name = match id_type {
        "OBJECT" => "Object",
        "MESH" => "Mesh",
        "MATERIAL" => "Material",
        "KEY" => "Key",
        "NODETREE" => "ShaderNodeTree",
        "CAMERA" => "Camera",
        "LIGHT" => "Light",
        "WORLD" => "World",
        "SCENE" => "Scene",
        "ARMATURE" => "Armature",
        "TEXTURE" => "Texture",
        "IMAGE" => "Image",
        "POINTCLOUD" => "PointCloud",
        "CURVE" => "Curve",
        "CURVES" => "Curves",
        "LATTICE" => "Lattice",
        "META" => "MetaBall",
        "SPEAKER" => "Speaker",
        "LIGHT_PROBE" => "LightProbe",
        "GREASEPENCIL" => "Annotation",
        "GREASEPENCIL_V3" => "GreasePencil",
        "VOLUME" => "Volume",
        "LINESTYLE" => "FreestyleLineStyle",
        "PARTICLE" => "ParticleSettings",
        "CACHEFILE" => "CacheFile",
        "MASK" => "Mask",
        "MOVIECLIP" => "MovieClip",
        "SOUND" => "Sound",
        "PALETTE" => "Palette",
        "BRUSH" => "Brush",
        _ => return None,
    };
    frap_spec_rna::find_struct(name)
}

/// `PROP_PROPORTIONAL` is not in the dumped definitions: the properties Blender 5.2 flags
/// (`RNA_def_property_flag(prop, PROP_PROPORTIONAL)` in `makesrna`).
fn is_proportional(owner: &'static StructDef, prop: &str) -> bool {
    const LIST: &[(&str, &str)] = &[
        ("Object", "scale"),
        ("Object", "delta_scale"),
        ("PoseBone", "scale"),
        ("PoseBone", "custom_shape_scale_xyz"),
        ("PoseBone", "custom_shape_translation"),
        ("PoseBone", "bbone_scalein"),
        ("PoseBone", "bbone_scaleout"),
        ("Bone", "bbone_scalein"),
        ("Bone", "bbone_scaleout"),
        ("EditBone", "bbone_scalein"),
        ("EditBone", "bbone_scaleout"),
        ("Mesh", "texspace_size"),
        ("Curve", "texspace_size"),
        ("MetaBall", "texspace_size"),
        ("UVProjectModifier", "aspect_x"),
        ("UVProjectModifier", "aspect_y"),
        ("UVProjectModifier", "scale_x"),
        ("UVProjectModifier", "scale_y"),
        ("Image", "generated_width"),
        ("Image", "generated_height"),
        ("UDIMTile", "generated_width"),
        ("UDIMTile", "generated_height"),
        ("RenderSettings", "resolution_x"),
        ("RenderSettings", "resolution_y"),
        ("RenderSettings", "pixel_aspect_x"),
        ("RenderSettings", "pixel_aspect_y"),
    ];
    LIST.iter().any(|(s, p)| *p == prop && owner.is_a(s))
}

/// Resolves `path` from `root` to the final property and the struct that owns it.
pub fn resolve(
    root: &'static StructDef,
    path: &str,
) -> Option<(&'static StructDef, &'static PropDef)> {
    let segs = parse_path(path)?;
    let mut cur = root;
    let mut i = 0;
    while i < segs.len() {
        let PathSeg::Prop(name) = &segs[i] else {
            return None;
        };
        let prop = cur.find_prop(name)?;
        let last = i + 1 == segs.len();
        match prop.kind {
            PropKind::Collection { item_type, .. } => {
                if last {
                    return None;
                }
                if !matches!(segs[i + 1], PathSeg::Index(_) | PathSeg::Key(_)) {
                    return None;
                }
                cur = frap_spec_rna::find_struct(item_type)?;
                i += 2;
            }
            PropKind::Pointer { fixed_type, .. } => {
                if last {
                    return None;
                }
                cur = frap_spec_rna::find_struct(fixed_type)?;
                i += 1;
            }
            _ => {
                return last.then_some((cur, prop));
            }
        }
    }
    None
}

/// The channel description of `path` from `root` (`None` when the definitions cannot follow
/// the path).
pub fn channel_prop(root: Option<&'static StructDef>, path: &str) -> Option<ChannelProp> {
    let (owner, prop) = resolve(root?, path)?;
    let is_array = !matches!(prop.array, ArrayShape::Scalar);
    let length = prop.array_len();
    let mix_mode = if prop.subtype == Subtype::Quaternion && length == 4 {
        MixMode::Quaternion
    } else if prop.subtype == Subtype::AxisAngle && length == 4 {
        MixMode::AxisAngle
    } else if is_proportional(owner, prop.identifier) {
        MixMode::Multiply
    } else {
        MixMode::Add
    };
    let mut defaults = match mix_mode {
        MixMode::Quaternion => vec![1.0, 0.0, 0.0, 0.0],
        MixMode::AxisAngle => vec![0.0; 4],
        _ => defaults_of(prop, length),
    };
    if mix_mode == MixMode::Multiply {
        for v in &mut defaults {
            if *v == 0.0 {
                *v = 1.0;
            }
        }
    }
    Some(ChannelProp {
        is_array,
        length,
        mix_mode,
        defaults,
        animatable: prop.flags.contains(PropFlags::ANIMATABLE),
    })
}

/// The RNA default values of `prop` as floats.
fn defaults_of(prop: &PropDef, length: usize) -> Vec<f32> {
    fn expand<T: Copy>(d: &Defaults<T>, n: usize, f: impl Fn(T) -> f32) -> Vec<f32> {
        match d {
            Defaults::Scalar(v) => vec![f(*v); n],
            Defaults::Array(a) => (0..n).map(|i| a.get(i).map_or(0.0, |v| f(*v))).collect(),
        }
    }
    match &prop.kind {
        PropKind::Bool { default } => expand(default, length, |b| if b { 1.0 } else { 0.0 }),
        PropKind::Int { default, .. } => expand(default, length, |i| i as f32),
        PropKind::Float { default, .. } => expand(default, length, |f| f as f32),
        PropKind::Enum { items, default, .. } => {
            let v = match default {
                EnumDefault::Item(id) => items
                    .iter()
                    .find(|i| i.identifier == *id)
                    .map_or(0, |i| i.value),
                EnumDefault::Flags(ids) => items
                    .iter()
                    .filter(|i| ids.contains(&i.identifier))
                    .fold(0, |acc, i| acc | i.value),
            };
            vec![v as f32; length]
        }
        _ => vec![0.0; length],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_parse() {
        assert_eq!(
            parse_path("pose.bones[\"Bo\\\"ne\"].location"),
            Some(vec![
                PathSeg::Prop("pose".into()),
                PathSeg::Prop("bones".into()),
                PathSeg::Key("Bo\"ne".into()),
                PathSeg::Prop("location".into()),
            ])
        );
        assert_eq!(
            parse_path("nodes['A'].inputs[0].default_value").map(|v| v.len()),
            Some(5)
        );
        assert_eq!(parse_path("location."), None);
        assert_eq!(parse_path("[0]"), None);
        assert_eq!(parse_path(""), None);
    }

    #[test]
    fn object_channels() {
        let ob = root_struct("OBJECT");
        let loc = channel_prop(ob, "location").expect("location");
        assert_eq!((loc.length, loc.mix_mode), (3, MixMode::Add));
        assert_eq!(loc.defaults, vec![0.0; 3]);
        let scale = channel_prop(ob, "scale").expect("scale");
        assert_eq!(scale.mix_mode, MixMode::Multiply);
        assert_eq!(scale.defaults, vec![1.0; 3]);
        let q = channel_prop(ob, "rotation_quaternion").expect("quat");
        assert_eq!(q.mix_mode, MixMode::Quaternion);
        assert_eq!(q.defaults, vec![1.0, 0.0, 0.0, 0.0]);
        let aa = channel_prop(ob, "rotation_axis_angle").expect("axis angle");
        assert_eq!(aa.mix_mode, MixMode::AxisAngle);
        let pb = channel_prop(ob, "pose.bones[\"Bone\"].scale").expect("pose bone");
        assert_eq!(pb.mix_mode, MixMode::Multiply);
        assert!(channel_prop(ob, "modifiers[\"Bevel\"].width").is_none());
        let key = channel_prop(root_struct("KEY"), "key_blocks[\"Key 1\"].value").expect("key");
        assert!(!key.is_array);
    }
}
