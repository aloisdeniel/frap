// SPDX-License-Identifier: GPL-3.0-or-later
//! COLOR_n writer (`export_vertex_color` MATERIAL/ACTIVE/NAME/NONE, `export_vertex_color_name`,
//! `export_all_vertex_colors`, `export_active_vertex_color_when_no_material`), with the fake
//! white `COLOR_0`.
//!
//! Ported from blender/scripts/addons_core/io_scene_gltf2/blender/exp/primitive_extract.py
//! (`manage_material_info`, the vertex-colour half, and `__manage_color_attributes`,
//! `__get_color_attribute_data`), blender/exp/material/materials.py (`get_base_material`: no
//! exported material ⇒ the active colour with alpha when
//! `export_active_vertex_color_when_no_material`) and primitive_attributes.py
//! (`__gather_attribute`: 4-component colours as normalized UNSIGNED_SHORT, the fake colour as
//! normalized UNSIGNED_BYTE), io_scene_gltf2 5.2.41, Apache-2.0, The glTF-Blender-IO authors.
//!
//! The plan ([`plan`]) walks the used material indices (ascending, one bucket `-1` with
//! `export_materials='NONE'`), gives each glTF colour set an index (`COLOR_<i>`) and records
//! which material buckets use it:
//! - MATERIAL: the colour attribute the material's base colour reads
//!   ([`LoweredMaterial::vertex_color`]: a named one, or the render colour for the active one);
//!   a material without one exports none (Blender warns that the active colour is not used);
//!   no exported material (none in the slot, `PLACEHOLDER`, `NONE`) means the render colour
//!   with alpha, unless `export_active_vertex_color_when_no_material` is off;
//! - ACTIVE / NAME: the render colour / the named colour (with alpha) for every bucket;
//! - with `export_all_vertex_colors` (default on, off with NONE): a fake `COLOR_0` (four
//!   normalized bytes of 255) when materials use no colour but the mesh has some, then every
//!   colour attribute not exported yet, with alpha;
//! - a material bucket that does not use a colour set gets white (`1.0`) in it, so the set
//!   does not tint that material.
//!
//! Every set is part of the split key (the add-on's `dots` fields, after every other
//! attribute): RGB(A) as f32 bits of the RNA `color` value (BYTE_COLOR decoded from sRGB to
//! linear), a POINT-domain colour read through each corner's vertex; the fake colour as one
//! all-ones word. Three components export as FLOAT VEC3, four as normalized UNSIGNED_SHORT
//! VEC4 (clipped to `[0, 1]`). Loose edges and points get the POINT-domain sets (unmasked) and
//! the fake one; CORNER-domain sets do not exist on them.
//!
//! In MATERIAL mode a set from a material adds alpha when the material's Alpha reads a colour
//! attribute and the material is not OPAQUE ([`frap_material::VertexColorUse::adds_alpha`]); a
//! material whose Alpha alone reads one exports no set (not managed by the add-on).

use std::collections::BTreeMap;

use frap_mesh::{AttrData, Domain, Mesh};
use frap_rna::{IdRef, MaterialId};
use frap_spec_ops_io::export_scene::gltf::ExportVertexColor;
use smol_str::SmolStr;

use crate::export::mesh::convert::{byte_color_to_linear, color_to_ushort, key_word};
use crate::export::mesh::ext::{
    AttrColumn, AttrValues, MeshExportCx, PRIMITIVE_ATTRIBUTE_WRITERS, PrimitiveAttributeWriter,
    PrimitiveDraft, SplitKey,
};
use crate::export::mesh::gather::{material_for_slot, slot_materials};
use crate::export::{ExportError, ExportMaterials};

/// This writer's split-key column.
pub const NAME: &str = "color";

/// The props honoured here.
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::export::SUPPORTED_EXPORT_PROPS)]
static PROPS: &[&str] = &[
    "export_vertex_color",
    "export_vertex_color_name",
    "export_all_vertex_colors",
    "export_active_vertex_color_when_no_material",
];

/// Where a material reads a colour attribute (`colorAttribType`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VcRef {
    /// `'active'`: the render colour attribute.
    Active,
    /// `'name'`: a named colour attribute.
    Name(SmolStr),
}

/// `material_info['vc_info']` of one material bucket.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VcInfo {
    /// Whether a glTF material is exported for the bucket (`base_material is not None`).
    pub has_material: bool,
    /// `color` / `color_type`.
    pub color: Option<VcRef>,
    /// `alpha` / `alpha_type`.
    pub alpha: Option<VcRef>,
    /// `alpha_mode == 'OPAQUE'`.
    pub alpha_opaque: bool,
}

impl VcInfo {
    /// No material, no colour.
    fn none(has_material: bool) -> VcInfo {
        VcInfo {
            has_material,
            color: None,
            alpha: None,
            alpha_opaque: true,
        }
    }
}

/// One exported colour set (`vc_infos` entry).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VcSet {
    /// glTF set index (`COLOR_<index>`).
    pub index: usize,
    /// The fake white colour (`forced`).
    pub forced: bool,
    /// Colour attribute (`color`).
    pub color: SmolStr,
    /// Alpha attribute (`alpha`).
    pub alpha: SmolStr,
    /// Four components (`add_alpha`).
    pub add_alpha: bool,
}

/// The vertex-colour plan of one mesh.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Plan {
    /// Colour sets in `vc_infos` order.
    pub sets: Vec<VcSet>,
    /// Material bucket (`-1` for 'NONE') → indices of the sets it uses.
    pub using: BTreeMap<i64, Vec<usize>>,
    /// The add-on's `export_settings['log'].warning`s of `manage_material_info`, in order
    /// (WP 7.7: reported through [`MeshExportCx::warn`]).
    pub warnings: Vec<&'static str>,
}

/// `manage_material_info`: two materials of the mesh use different colour sets (once per
/// mesh, `warning_already_displayed`).
pub const WARN_DIFFERENT_COLORS: &str =
    "glTF specification does not allow this case (multiple materials with different Vertex Color)";
/// `manage_material_info`: with `export_vertex_color='MATERIAL'`, a material reads no colour
/// attribute while the mesh has an active one (once per mesh).
pub const WARN_ACTIVE_NOT_USED: &str = "The active Vertex Color will not be exported, as it is not used in the node tree of the material";
/// `manage_material_info`: a material's alpha reads a colour attribute, its colour none (each
/// such material).
pub const WARN_ALPHA_WITHOUT_COLOR: &str =
    "We are not managing this case for now (Vertex Color alpha without color)";

/// The settings the plan reads (`export_settings['gltf_vertex_color']`, …, after the add-on's
/// coupling: with NONE neither `all` nor `when_no_material` applies; the name only with NAME).
#[derive(Clone, Debug)]
pub struct VcSettings {
    /// `gltf_vertex_color`
    pub mode: ExportVertexColor,
    /// `gltf_vertex_color_name`
    pub name: SmolStr,
    /// `gltf_all_vertex_colors`
    pub all: bool,
    /// `gltf_active_vertex_color_when_no_material`
    pub when_no_material: bool,
}

impl VcSettings {
    /// From the export settings.
    pub fn of(cx: &MeshExportCx) -> VcSettings {
        let p = &cx.settings.props;
        let mode = p.export_vertex_color.value;
        let none = mode == ExportVertexColor::None;
        VcSettings {
            mode,
            name: if mode == ExportVertexColor::Name {
                p.export_vertex_color_name.value.clone()
            } else {
                SmolStr::default()
            },
            all: !none && p.export_all_vertex_colors.value,
            when_no_material: !none && p.export_active_vertex_color_when_no_material.value,
        }
    }
}

/// `mesh.color_attributes` names, in attribute order: colour-typed POINT/CORNER attributes
/// that are not internal.
pub fn color_attributes(mesh: &Mesh) -> Vec<SmolStr> {
    mesh.attributes()
        .iter()
        .filter(|a| {
            matches!(a.data, AttrData::ByteColor(_) | AttrData::FloatColor(_))
                && matches!(a.domain, Domain::Point | Domain::Corner)
                && !a.name.starts_with('.')
        })
        .map(|a| a.name.clone())
        .collect()
}

/// `color_attributes[render_color_index].name` (`None` when there is no render colour).
fn render_color(mesh: &Mesh, colors: &[SmolStr]) -> Option<SmolStr> {
    mesh.color_default
        .as_ref()
        .filter(|n| colors.contains(n))
        .cloned()
}

/// The `manage_material_info` vertex-colour walk plus the `export_all_vertex_colors` pass.
/// `buckets` are the material buckets in `prim_indices` order with their `vc_info`.
pub fn plan(
    s: &VcSettings,
    colors: &[SmolStr],
    render: Option<&SmolStr>,
    active: Option<&SmolStr>,
    buckets: &[(i64, VcInfo)],
) -> Plan {
    let mut p = Plan::default();
    let mut warned_different = false;
    let mut warned_active = false;
    let mut next = 0usize;
    let mut materials_use_vc: Option<String> = None;
    let mut no_materials = true;
    let find = |n: &str| colors.iter().any(|c| c == n);
    let named = || find(&s.name).then(|| s.name.clone());
    for (idx, info) in buckets {
        let using = p.using.entry(*idx).or_default();
        if info.has_material {
            no_materials = false;
        }
        if s.mode == ExportVertexColor::None {
            continue;
        }
        let (color, alpha, add_alpha_rule): (Option<SmolStr>, Option<SmolStr>, bool) =
            if info.color.is_none() && info.alpha.is_none() {
                // No colour in the node tree.
                let forced = (info.has_material
                    && matches!(s.mode, ExportVertexColor::Active | ExportVertexColor::Name))
                    || (!info.has_material && s.when_no_material);
                if !forced {
                    if info.has_material
                        && s.mode == ExportVertexColor::Material
                        && !warned_active
                        && active.is_some()
                    {
                        p.warnings.push(WARN_ACTIVE_NOT_USED);
                        warned_active = true;
                    }
                    continue;
                }
                let n = if (info.has_material && s.mode == ExportVertexColor::Active)
                    || (!info.has_material && s.when_no_material)
                {
                    render.cloned()
                } else if info.has_material && s.mode == ExportVertexColor::Name {
                    named()
                } else {
                    None
                };
                (n.clone(), n, true)
            } else if info.color.is_none() {
                // Alpha without colour: not managed by the add-on.
                p.warnings.push(WARN_ALPHA_WITHOUT_COLOR);
                continue;
            } else {
                let resolve = |r: &Option<VcRef>| match r {
                    Some(VcRef::Name(n)) => Some(n.clone()),
                    Some(VcRef::Active) => render.cloned(),
                    None => None,
                };
                let (c, a) = match s.mode {
                    ExportVertexColor::Name => (named(), named()),
                    ExportVertexColor::Active => (render.cloned(), render.cloned()),
                    _ => (resolve(&info.color), resolve(&info.alpha)),
                };
                let mut add = a.is_some();
                if !matches!(s.mode, ExportVertexColor::Name | ExportVertexColor::Active) {
                    add = add && !info.alpha_opaque;
                }
                (c, a, add)
            };
        let Some(color) = color else {
            continue;
        };
        let key = format!("{color}{}", alpha.as_deref().unwrap_or(""));
        match &materials_use_vc {
            // glTF cannot express materials with different colour sets: the later set is
            // neither exported nor used (the add-on warns).
            Some(k) if *k != key => {
                if !warned_different {
                    p.warnings.push(WARN_DIFFERENT_COLORS);
                    warned_different = true;
                }
                materials_use_vc = Some(key);
            }
            None => {
                materials_use_vc = Some(key);
                p.sets.push(VcSet {
                    index: next,
                    forced: false,
                    color,
                    alpha: alpha.unwrap_or_default(),
                    add_alpha: add_alpha_rule,
                });
                using.push(next);
                next += 1;
            }
            Some(_) => using.push(next - 1),
        }
    }
    if s.all {
        if !no_materials && p.sets.is_empty() && !colors.is_empty() {
            p.sets.push(VcSet {
                index: next,
                forced: true,
                color: SmolStr::default(),
                alpha: SmolStr::default(),
                add_alpha: true,
            });
            for u in p.using.values_mut() {
                u.push(next);
            }
            next += 1;
        }
        if !no_materials || s.when_no_material {
            for c in colors {
                if p.sets.iter().any(|v| !v.forced && v.color == *c) {
                    continue;
                }
                p.sets.push(VcSet {
                    index: next,
                    forced: false,
                    color: c.clone(),
                    alpha: c.clone(),
                    add_alpha: true,
                });
                for u in p.using.values_mut() {
                    u.push(next);
                }
                next += 1;
            }
        }
    }
    p
}

/// The material bucket of every face (`loop_triangles[i].material_index`, `-1` with 'NONE').
fn face_buckets(cx: &MeshExportCx) -> Vec<i64> {
    let n = cx.mesh.faces_num();
    if cx.settings.export_materials == ExportMaterials::None {
        return vec![-1; n];
    }
    let mi = cx
        .mesh
        .attributes()
        .get("material_index")
        .and_then(|a| match (&a.domain, &a.data) {
            (Domain::Face, AttrData::Int(v)) => Some(v.clone()),
            _ => None,
        });
    (0..n)
        .map(|f| {
            mi.as_ref()
                .and_then(|v| v.get(f).copied())
                .map_or(0, |i| i64::from(i as u32))
        })
        .collect()
}

/// `get_base_material(...)[2]['vc_info']` of one bucket.
fn vc_info(cx: &MeshExportCx, mats: &[Option<IdRef<MaterialId>>], bucket: i64) -> VcInfo {
    let s = cx.settings;
    let slot = usize::try_from(bucket).unwrap_or(usize::MAX);
    let mat =
        material_for_slot(s.export_materials, slot, mats).and_then(|m| cx.main.materials.get(m));
    let none_or_active = |has_material| {
        if VcSettings::of(cx).when_no_material {
            VcInfo {
                has_material,
                color: Some(VcRef::Active),
                alpha: Some(VcRef::Active),
                alpha_opaque: false,
            }
        } else {
            VcInfo::none(has_material)
        }
    };
    let Some(mat) = mat else {
        return none_or_active(false);
    };
    match s.export_materials {
        ExportMaterials::Export => {
            let lm = crate::export::materials::lower::lowered_for(cx.main, mat, s);
            match lm.vertex_color {
                None => VcInfo::none(true),
                Some(v) => {
                    let r = |n: &Option<SmolStr>| match n {
                        Some(n) if !n.is_empty() => VcRef::Name(n.clone()),
                        _ => VcRef::Active,
                    };
                    // `export_image_format='NONE'`: `__gather_base_color_factor` skips
                    // `gather_color_info`, so the colour attribute is not used (WP 7.7).
                    let image_none =
                        s.export_image_format == crate::export::settings::ExportImageFormat::None;
                    VcInfo {
                        has_material: true,
                        color: (v.color && !image_none).then(|| r(&v.attribute)),
                        alpha: v.alpha.then(|| r(&v.alpha_attribute)),
                        alpha_opaque: lm.alpha_mode == frap_material::AlphaMode::Opaque,
                    }
                }
            }
        }
        ExportMaterials::Viewport => VcInfo::none(true),
        // `__filter_material` is false: no material, so the active colour applies.
        ExportMaterials::Placeholder | ExportMaterials::None => none_or_active(false),
    }
}

/// The plan of the mesh being exported.
pub fn mesh_plan(cx: &MeshExportCx) -> Plan {
    let s = VcSettings::of(cx);
    let colors = color_attributes(cx.mesh);
    let render = render_color(cx.mesh, &colors);
    let mats = match cx.object.and_then(|o| cx.main.objects.get(o)) {
        Some(ob) => slot_materials(ob, cx.mesh),
        None => cx.mesh.materials.clone(),
    };
    // `prim_indices` keys: the buckets of faces with triangles, ascending.
    let mut used: Vec<i64> = face_buckets(cx)
        .into_iter()
        .enumerate()
        .filter(|(f, _)| cx.mesh.face(*f).len() >= 3)
        .map(|(_, b)| b)
        .collect();
    used.sort_unstable();
    used.dedup();
    let buckets: Vec<(i64, VcInfo)> = used
        .into_iter()
        .map(|b| (b, vc_info(cx, &mats, b)))
        .collect();
    let active = cx.mesh.color_active.as_ref().filter(|n| colors.contains(n));
    plan(&s, &colors, render.as_ref(), active, &buckets)
}

/// RNA `color` values of a colour attribute in its own domain (BYTE_COLOR decoded to linear).
fn colour_values(mesh: &Mesh, name: &str) -> Option<(Domain, Vec<[f32; 4]>)> {
    let a = mesh.attributes().get(name)?;
    let v = match &a.data {
        AttrData::FloatColor(v) => v.to_vec(),
        AttrData::ByteColor(v) => v.iter().map(|c| byte_color_to_linear(*c)).collect(),
        _ => return None,
    };
    Some((a.domain, v))
}

/// A set's values at each Blender corner (alpha from its alpha attribute when it has one),
/// before material masking.
fn corner_values(mesh: &Mesh, set: &VcSet) -> Option<Vec<[f32; 4]>> {
    let cv = mesh.corner_verts();
    let at = |(d, v): (Domain, Vec<[f32; 4]>)| -> Vec<[f32; 4]> {
        match d {
            Domain::Point => cv.iter().map(|&x| v[x as usize]).collect(),
            _ => v,
        }
    };
    let mut c = at(colour_values(mesh, &set.color)?);
    if set.add_alpha
        && let Some(a) = colour_values(mesh, &set.alpha)
    {
        for (x, y) in c.iter_mut().zip(at(a)) {
            x[3] = y[3];
        }
    }
    Some(c)
}

/// Stride (words per corner) of a set in the key.
fn stride(set: &VcSet) -> usize {
    if set.forced {
        1
    } else if set.add_alpha {
        4
    } else {
        3
    }
}

/// The sets that `__manage_color_attributes` writes (a missing attribute is skipped; its glTF
/// index stays taken).
fn written_sets(mesh: &Mesh, plan: &Plan) -> Vec<VcSet> {
    plan.sets
        .iter()
        .filter(|s| s.forced || colour_values(mesh, &s.color).is_some())
        .cloned()
        .collect()
}

fn key(cx: &MeshExportCx, k: &mut SplitKey) -> Result<(), ExportError> {
    let plan = mesh_plan(cx);
    for w in &plan.warnings {
        cx.warn(*w);
    }
    let sets = written_sets(cx.mesh, &plan);
    if sets.is_empty() {
        return Ok(());
    }
    let n = cx.mesh.corners_num();
    let buckets = face_buckets(cx);
    let mut corner_bucket = vec![0i64; n];
    for (f, b) in buckets.iter().enumerate() {
        corner_bucket[cx.mesh.face(f)].fill(*b);
    }
    let width: usize = sets.iter().map(stride).sum();
    let mut words = vec![0u32; width * n];
    let mut off = 0;
    for set in &sets {
        let w = stride(set);
        if set.forced {
            for c in 0..n {
                words[c * width + off] = u32::MAX;
            }
        } else {
            let mut vals = corner_values(cx.mesh, set).unwrap_or_default();
            for (c, v) in vals.iter_mut().enumerate() {
                let uses = plan
                    .using
                    .get(&corner_bucket[c])
                    .is_some_and(|u| u.contains(&set.index));
                if !uses {
                    v[0] = 1.0;
                    v[1] = 1.0;
                    v[2] = 1.0;
                    if set.add_alpha {
                        v[3] = 1.0;
                    }
                }
                for i in 0..w {
                    words[c * width + off + i] = key_word(v[i]);
                }
            }
        }
        off += w;
    }
    k.push(NAME, width, words);
    Ok(())
}

/// The glTF column of a set from its values.
fn column(set: &VcSet, values: Vec<[f32; 4]>) -> AttrColumn {
    if set.forced {
        AttrColumn {
            values: AttrValues::U8x4(vec![[255; 4]; values.len()]),
            normalized: true,
        }
    } else if set.add_alpha {
        AttrColumn {
            values: AttrValues::U16x4(values.into_iter().map(color_to_ushort).collect()),
            normalized: true,
        }
    } else {
        AttrColumn {
            values: AttrValues::F32x3(values.into_iter().map(|c| [c[0], c[1], c[2]]).collect()),
            normalized: false,
        }
    }
}

fn run(cx: &MeshExportCx, k: &SplitKey, d: &mut PrimitiveDraft) -> Result<(), ExportError> {
    let Some(col) = k.column(NAME) else {
        return Ok(());
    };
    let plan = mesh_plan(cx);
    let sets = written_sets(cx.mesh, &plan);
    let loose = d.corners.contains(&u32::MAX);
    let mut off = 0;
    for set in &sets {
        let w = stride(set);
        let name: SmolStr = format!("COLOR_{}", set.index).into();
        if !loose {
            let values = d
                .corners
                .iter()
                .map(|&c| {
                    let at = c as usize * col.stride + off;
                    let mut v = [1.0f32; 4];
                    if !set.forced {
                        for (i, x) in v.iter_mut().enumerate().take(w) {
                            *x = f32::from_bits(col.words[at + i]);
                        }
                    }
                    v
                })
                .collect();
            d.attributes.insert(name, column(set, values));
        } else if set.forced {
            d.attributes
                .insert(name, column(set, vec![[1.0; 4]; d.vertices.len()]));
        } else if let Some((Domain::Point, vals)) = colour_values(cx.mesh, &set.color) {
            let alpha = if set.add_alpha {
                colour_values(cx.mesh, &set.alpha).filter(|(d, _)| *d == Domain::Point)
            } else {
                None
            };
            let values = d
                .vertices
                .iter()
                .map(|&v| {
                    let mut c = vals[v as usize];
                    if let Some((_, a)) = &alpha {
                        c[3] = a[v as usize][3];
                    }
                    c
                })
                .collect();
            d.attributes.insert(name, column(set, values));
        }
        off += w;
    }
    Ok(())
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(PRIMITIVE_ATTRIBUTE_WRITERS)]
static COLOR: PrimitiveAttributeWriter = PrimitiveAttributeWriter {
    name: NAME,
    order: 400,
    key: Some(key),
    run,
};

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use frap_mesh::Attribute;
    use frap_scene::Main;

    use super::*;
    use crate::export::ExportSettings;
    use crate::export::mesh::primitives;

    fn vs(mode: ExportVertexColor, name: &str, all: bool, nomat: bool) -> VcSettings {
        VcSettings {
            mode,
            name: name.into(),
            all,
            when_no_material: nomat,
        }
    }

    fn names(p: &Plan) -> Vec<(usize, bool, &str, bool)> {
        p.sets
            .iter()
            .map(|s| (s.index, s.forced, s.color.as_str(), s.add_alpha))
            .collect()
    }

    const COLS: [&str; 2] = ["Col", "PCol"];

    fn cols() -> Vec<SmolStr> {
        COLS.iter().map(|c| SmolStr::new(*c)).collect()
    }

    #[test]
    fn material_without_colour_gets_the_fake_set_then_every_colour() {
        // Blender 5.2.4: factory cube + two colour attributes, defaults.
        let m = VcInfo::none(true);
        let p = plan(
            &vs(ExportVertexColor::Material, "", true, true),
            &cols(),
            Some(&"Col".into()),
            None,
            &[(0, m)],
        );
        assert_eq!(
            names(&p),
            [
                (0, true, "", true),
                (1, false, "Col", true),
                (2, false, "PCol", true)
            ]
        );
        assert_eq!(p.using[&0], [0, 1, 2]);
        // export_all_vertex_colors off: nothing.
        let p = plan(
            &vs(ExportVertexColor::Material, "", false, true),
            &cols(),
            Some(&"Col".into()),
            None,
            &[(0, VcInfo::none(true))],
        );
        assert!(p.sets.is_empty());
    }

    #[test]
    fn active_and_name_modes() {
        let r: SmolStr = "Col".into();
        let p = plan(
            &vs(ExportVertexColor::Active, "", true, true),
            &cols(),
            Some(&r),
            None,
            &[(0, VcInfo::none(true))],
        );
        assert_eq!(
            names(&p),
            [(0, false, "Col", true), (1, false, "PCol", true)]
        );
        let p = plan(
            &vs(ExportVertexColor::Name, "PCol", true, true),
            &cols(),
            Some(&r),
            None,
            &[(0, VcInfo::none(true))],
        );
        assert_eq!(
            names(&p),
            [(0, false, "PCol", true), (1, false, "Col", true)]
        );
        // A name that does not exist exports no named set (the others still follow).
        let p = plan(
            &vs(ExportVertexColor::Name, "Nope", true, true),
            &cols(),
            Some(&r),
            None,
            &[(0, VcInfo::none(true))],
        );
        assert_eq!(names(&p)[0], (0, true, "", true));
        // NONE: nothing at all.
        let p = plan(
            &vs(ExportVertexColor::None, "", false, false),
            &cols(),
            Some(&r),
            None,
            &[(0, VcInfo::none(true))],
        );
        assert!(p.sets.is_empty());
    }

    #[test]
    fn no_material_uses_the_render_colour_with_alpha() {
        let active = VcInfo {
            has_material: false,
            color: Some(VcRef::Active),
            alpha: Some(VcRef::Active),
            alpha_opaque: false,
        };
        let p = plan(
            &vs(ExportVertexColor::Material, "", true, true),
            &cols(),
            Some(&"PCol".into()),
            None,
            &[(0, active)],
        );
        assert_eq!(
            names(&p),
            [(0, false, "PCol", true), (1, false, "Col", true)]
        );
        // Off: no material and no colour.
        let p = plan(
            &vs(ExportVertexColor::Material, "", true, false),
            &cols(),
            Some(&"PCol".into()),
            None,
            &[(0, VcInfo::none(false))],
        );
        assert!(p.sets.is_empty());
    }

    #[test]
    fn material_colour_and_masking_of_other_materials() {
        let uses = VcInfo {
            has_material: true,
            color: Some(VcRef::Name("PCol".into())),
            alpha: None,
            alpha_opaque: true,
        };
        let p = plan(
            &vs(ExportVertexColor::Material, "", true, true),
            &cols(),
            Some(&"Col".into()),
            None,
            &[(0, uses.clone()), (1, VcInfo::none(true))],
        );
        // PCol without alpha (OPAQUE), used by bucket 0 only; Col added for both.
        assert_eq!(
            names(&p),
            [(0, false, "PCol", false), (1, false, "Col", true)]
        );
        assert_eq!(p.using[&0], [0, 1]);
        assert_eq!(p.using[&1], [1]);
        // Two materials with different colours: only the first set exists.
        let other = VcInfo {
            color: Some(VcRef::Name("Col".into())),
            ..uses.clone()
        };
        let p = plan(
            &vs(ExportVertexColor::Material, "", false, true),
            &cols(),
            None,
            None,
            &[(0, uses), (1, other)],
        );
        assert_eq!(names(&p), [(0, false, "PCol", false)]);
        assert!(p.using[&1].is_empty());
        assert_eq!(p.warnings, [WARN_DIFFERENT_COLORS]);
    }

    /// WP 7.7: `manage_material_info`'s warnings (Blender 5.2.4 log, `followups/vc_warnings`).
    #[test]
    fn vertex_colour_warnings() {
        let col: SmolStr = "Col".into();
        let mat = vs(ExportVertexColor::Material, "", true, true);
        // A material without colour while the mesh has an active colour: once per mesh.
        let p = plan(
            &mat,
            &cols(),
            Some(&col),
            Some(&col),
            &[(0, VcInfo::none(true)), (1, VcInfo::none(true))],
        );
        assert_eq!(p.warnings, [WARN_ACTIVE_NOT_USED]);
        // No active colour attribute: silent; ACTIVE mode: silent.
        let p = plan(&mat, &cols(), Some(&col), None, &[(0, VcInfo::none(true))]);
        assert!(p.warnings.is_empty());
        let p = plan(
            &vs(ExportVertexColor::Active, "", true, true),
            &cols(),
            Some(&col),
            Some(&col),
            &[(0, VcInfo::none(true))],
        );
        assert!(p.warnings.is_empty());
        // Alpha without colour: each such material.
        let alpha_only = VcInfo {
            has_material: true,
            color: None,
            alpha: Some(VcRef::Name("Col".into())),
            alpha_opaque: false,
        };
        let p = plan(
            &mat,
            &cols(),
            Some(&col),
            None,
            &[(0, alpha_only.clone()), (1, alpha_only)],
        );
        assert_eq!(
            p.warnings,
            [WARN_ALPHA_WITHOUT_COLOR, WARN_ALPHA_WITHOUT_COLOR]
        );
        // Three materials with three colours: warned once.
        let using = |n: &str| VcInfo {
            has_material: true,
            color: Some(VcRef::Name(n.into())),
            alpha: None,
            alpha_opaque: true,
        };
        let p = plan(
            &mat,
            &cols(),
            None,
            None,
            &[(0, using("Col")), (1, using("PCol")), (2, using("Col"))],
        );
        assert_eq!(p.warnings, [WARN_DIFFERENT_COLORS]);
    }

    fn coloured_cube() -> Mesh {
        let mut m = frap_mesh::primitives::cube(2.0, true, [1.0; 3]);
        let n = m.corners_num();
        let v = m.verts_num();
        m.attributes_mut().insert(Attribute {
            name: "Col".into(),
            domain: Domain::Corner,
            data: AttrData::ByteColor(Arc::new(
                (0..n).map(|i| [(i * 10) as u8, 0, 255, 128]).collect(),
            )),
        });
        m.attributes_mut().insert(Attribute {
            name: "PCol".into(),
            domain: Domain::Point,
            data: AttrData::FloatColor(Arc::new(
                (0..v).map(|i| [i as f32 / 8.0, 0.5, 2.0, 1.0]).collect(),
            )),
        });
        m.color_default = Some("Col".into());
        m.color_active = Some("Col".into());
        m
    }

    #[test]
    fn cube_without_material_exports_render_colour_first() {
        let m = coloured_cube();
        let main = Main::default();
        let s = ExportSettings::default();
        let cx = MeshExportCx::new(&main, &m, None, &s);
        let d = primitives::extract(&cx).unwrap();
        assert_eq!(d.len(), 1);
        let keys: Vec<_> = d[0].attributes.keys().map(|k| k.as_str()).collect();
        assert_eq!(keys, ["COLOR_0", "COLOR_1"]);
        let AttrValues::U16x4(c0) = &d[0].attributes["COLOR_0"].values else {
            panic!("BYTE colour with alpha is UNSIGNED_SHORT VEC4")
        };
        // Corner colours split the 24 corners into 24 vertices (they already were).
        assert_eq!(c0.len(), 24);
        let AttrValues::U16x4(c1) = &d[0].attributes["COLOR_1"].values else {
            panic!("FLOAT colour with alpha is UNSIGNED_SHORT VEC4 too")
        };
        // FLOAT_COLOR values are clipped; vertex 0 is (0, .5, 2 → 1, 1).
        let v0 = d[0].vertices.iter().position(|&v| v == 0).unwrap();
        assert_eq!(c1[v0], [0, 32768, 65535, 65535]);
    }

    #[test]
    fn none_and_loose_points() {
        let mut m = coloured_cube();
        let main = Main::default();
        let mut p = crate::export::settings::default_props();
        p.export_vertex_color.value = ExportVertexColor::None;
        let s = ExportSettings {
            props: Arc::new(p),
            ..ExportSettings::default()
        };
        let cx = MeshExportCx::new(&main, &m, None, &s);
        let d = primitives::extract(&cx).unwrap();
        assert!(d[0].attributes.is_empty());

        // A loose vertex: only the POINT colour reaches the POINTS primitive.
        let mut pos = m.positions().to_vec();
        pos.push([5.0, 5.0, 5.0]);
        m.set_positions(pos);
        for a in m.attributes_mut().iter_mut() {
            if a.name == "PCol"
                && let AttrData::FloatColor(v) = &mut a.data
            {
                Arc::make_mut(v).push([0.25, 0.0, 0.0, 0.5]);
            }
        }
        let mut p = crate::export::settings::default_props();
        p.use_mesh_vertices.value = true;
        let s = ExportSettings {
            props: Arc::new(p),
            ..ExportSettings::default()
        };
        let cx = MeshExportCx::new(&main, &m, None, &s);
        let d = primitives::extract(&cx).unwrap();
        assert_eq!(d.len(), 2);
        let keys: Vec<_> = d[1].attributes.keys().map(|k| k.as_str()).collect();
        assert_eq!(keys, ["COLOR_1"]);
        let AttrValues::U16x4(c) = &d[1].attributes["COLOR_1"].values else {
            panic!()
        };
        assert_eq!(c[0], color_to_ushort([0.25, 0.0, 0.0, 0.5]));
    }
}
