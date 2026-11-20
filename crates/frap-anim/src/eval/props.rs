// SPDX-License-Identifier: GPL-3.0-or-later
//! What the NLA needs to know about an animated property without the data
//! (`nlaevalchan_verify` → `RNA_path_resolve_property`, `nlaevalchan_detect_mix_mode`,
//! `nlaevalchan_get_default_values` in blender/source/blender/blenkernel/intern/anim_sys.cc,
//! Blender 5.2.4): the F-curve's `data_path` is walked through the generated RNA definitions
//! from the animated ID's struct, giving the array length, the mix mode (quaternion, axis-angle,
//! multiplicative, additive) and the RNA default values the NLA blends from.
//!
//! Collection items and pointers whose struct is refined from the data (a modifier's type,
//! a node's `bl_idname`, a socket's type) are resolved through a [`Refine`] callback the
//! caller supplies. A path neither can follow (no callback, or an unknown item: e.g.
//! `modifiers["Bevel"].width` without one, whose item struct is the `Modifier` base) falls
//! back to one scalar channel per array index, additive, with default 0 (deviations §2).

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
    PROPORTIONAL
        .iter()
        .any(|(s, p)| *p == prop && owner.is_a(s))
}

/// The `(struct, property)` pairs flagged `PROP_PROPORTIONAL` in Blender 5.2.4's `makesrna`
/// (a test checks the list against the source when it is available).
const PROPORTIONAL: &[(&str, &str)] = &[
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
    ("TexMapping", "scale"),
    ("TextureSlot", "scale"),
    ("LineStyleGeometryModifier_2DTransform", "scale_x"),
    ("LineStyleGeometryModifier_2DTransform", "scale_y"),
    ("MetaElement", "size_x"),
    ("MetaElement", "size_y"),
    ("MetaElement", "size_z"),
];

/// The refined struct of a collection item or pointer reached by `prefix` (the path segments
/// up to and including the item key or the pointer property) whose definition names `base`
/// (`Modifier`, `Node`, `NodeSocket`, `ID`, …), as the data has it (`rna_Modifier_refine`,
/// `rna_Node_refine`, `rna_NodeSocket_refine`, …). `None` keeps `base`.
pub type Refine<'a> = &'a dyn Fn(&[PathSeg], &'static StructDef) -> Option<&'static StructDef>;

/// Resolves `path` from `root` to the final property and the struct that owns it.
pub fn resolve(
    root: &'static StructDef,
    path: &str,
) -> Option<(&'static StructDef, &'static PropDef)> {
    resolve_refined(root, path, None)
}

/// [`resolve`] refining collection items and pointers through `refine`.
pub fn resolve_refined(
    root: &'static StructDef,
    path: &str,
    refine: Option<Refine<'_>>,
) -> Option<(&'static StructDef, &'static PropDef)> {
    let refined = |prefix: &[PathSeg], base: &'static StructDef| {
        refine.and_then(|f| f(prefix, base)).unwrap_or(base)
    };
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
                cur = refined(&segs[..i + 2], frap_spec_rna::find_struct(item_type)?);
                i += 2;
            }
            PropKind::Pointer { fixed_type, .. } => {
                if last {
                    return None;
                }
                cur = refined(&segs[..i + 1], frap_spec_rna::find_struct(fixed_type)?);
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
    channel_prop_refined(root, path, None)
}

/// [`channel_prop`] refining collection items and pointers through `refine`.
pub fn channel_prop_refined(
    root: Option<&'static StructDef>,
    path: &str,
    refine: Option<Refine<'_>>,
) -> Option<ChannelProp> {
    let (owner, prop) = resolve_refined(root?, path, refine)?;
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

    #[test]
    fn refined_items() {
        let ob = root_struct("OBJECT");
        let refine = |prefix: &[PathSeg], base: &'static StructDef| {
            assert_eq!(base.identifier, "Modifier");
            assert_eq!(
                prefix,
                [
                    PathSeg::Prop("modifiers".into()),
                    PathSeg::Key("Bevel".into())
                ]
            );
            frap_spec_rna::find_struct("BevelModifier")
        };
        let w = channel_prop_refined(ob, "modifiers[\"Bevel\"].width", Some(&refine))
            .expect("bevel width");
        assert_eq!(
            (w.length, w.mix_mode, w.defaults),
            (1, MixMode::Add, vec![0.1])
        );
        // Unknown refinement: the base struct, which has no `width`.
        let none = |_: &[PathSeg], _: &'static StructDef| None;
        assert!(channel_prop_refined(ob, "modifiers[\"Bevel\"].width", Some(&none)).is_none());
        // A refined proportional property (`TexMapping.scale` through a texture node).
        let mat = root_struct("NODETREE");
        let tex = |p: &[PathSeg], base: &'static StructDef| {
            (p.len() == 2 && base.identifier == "Node")
                .then(|| frap_spec_rna::find_struct("ShaderNodeTexNoise"))
                .flatten()
        };
        let sc = channel_prop_refined(mat, "nodes[\"N\"].texture_mapping.scale", Some(&tex))
            .expect("texture mapping scale");
        assert_eq!(
            (sc.mix_mode, sc.defaults),
            (MixMode::Multiply, vec![1.0; 3])
        );
    }

    /// Every listed property of a struct the definitions hold exists (some structs, such as
    /// `Curve` or `MetaElement`, are not generated yet).
    #[test]
    fn proportional_entries_exist() {
        let mut checked = 0;
        for (s, p) in PROPORTIONAL {
            if let Some(def) = frap_spec_rna::find_struct(s) {
                assert!(def.find_prop(p).is_some(), "{s}.{p}");
                checked += 1;
            }
        }
        assert!(checked >= 25, "only {checked} entries checked");
    }

    /// The list against `RNA_def_property_flag(prop, PROP_PROPORTIONAL…)` in Blender's
    /// `makesrna` (`FRAP_BLENDER_SRC`, else the shared checkout; skipped when absent).
    #[test]
    fn proportional_list_matches_makesrna() {
        let root = std::env::var("FRAP_BLENDER_SRC")
            .unwrap_or_else(|_| "/private/tmp/claude-501/blender-src".into());
        let dir = std::path::Path::new(&root).join("source/blender/makesrna/intern");
        let Ok(entries) = std::fs::read_dir(&dir) else {
            eprintln!("skipped: no Blender source at {}", dir.display());
            return;
        };
        let quoted_after = |text: &str, needle: &str| -> Option<String> {
            let at = text.rfind(needle)? + needle.len();
            let rest = text[at..].trim_start_matches(|c: char| c.is_whitespace() || c == ',');
            let rest = rest.strip_prefix("brna,").map_or(rest, str::trim_start);
            let rest = rest.strip_prefix('"')?;
            Some(rest[..rest.find('"')?].to_owned())
        };
        let mut found: Vec<(Option<String>, String)> = Vec::new();
        for e in entries.flatten() {
            let path = e.path();
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default();
            if !name.starts_with("rna_")
                || !name.ends_with(".cc")
                || ["rna_access.cc", "rna_define.cc", "rna_rna.cc"].contains(&name)
            {
                continue;
            }
            let text = std::fs::read_to_string(&path).expect("read");
            let mut from = 0;
            while let Some(i) = text[from..].find("PROP_PROPORTIONAL") {
                let at = from + i;
                from = at + 1;
                let line_start = text[..at].rfind('\n').map_or(0, |n| n + 1);
                if !text[line_start..at].contains("RNA_def_property_flag(") {
                    continue;
                }
                let before = &text[..at];
                let prop = quoted_after(before, "RNA_def_property(srna,").expect("property");
                // The struct, when it is defined in the same function (shared helpers such
                // as `rna_def_bone_curved_common` serve several).
                let func = before.rfind("\nstatic void ").max(before.rfind("\nvoid "));
                let st = quoted_after(before, "RNA_def_struct(")
                    .filter(|_| before.rfind("RNA_def_struct(") > func);
                found.push((st, prop));
            }
        }
        assert!(found.len() > 20, "only {} flags found", found.len());
        for (st, prop) in &found {
            match st {
                Some(st) => assert!(
                    PROPORTIONAL.contains(&(st.as_str(), prop.as_str())),
                    "{st}.{prop} is PROP_PROPORTIONAL in Blender"
                ),
                None => assert!(
                    PROPORTIONAL.iter().any(|(_, p)| p == prop),
                    "*.{prop} is PROP_PROPORTIONAL in Blender"
                ),
            }
        }
        for (st, prop) in PROPORTIONAL {
            assert!(
                found
                    .iter()
                    .any(|(s, p)| p == prop && s.as_deref().is_none_or(|s| s == *st)),
                "{st}.{prop} is not PROP_PROPORTIONAL in Blender"
            );
        }
    }
}
