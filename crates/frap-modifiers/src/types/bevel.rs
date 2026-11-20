// SPDX-License-Identifier: GPL-3.0-or-later
//! BEVEL (`BevelModifier`): bevels edges (or vertices) chosen by `limit_method`, through a
//! BMesh and the shared bevel core of `mesh.bevel` (`BM_mesh_bevel`, `frap-bmesh`
//! `ops/bevel.rs`).
//!
//! Ported from blender/source/blender/modifiers/intern/MOD_bevel.cc (`modify_mesh`,
//! `is_disabled`, `ensure_weight_attribute_meta_data`; Blender 5.2.4, GPL-2.0-or-later).
//!
//! Which elements are beveled (`limit_method`):
//! - `affect='EDGES'`: `ANGLE` (the default) every edge whose two faces meet at more than
//!   `angle_limit`; `WEIGHT` every manifold edge whose `edge_weight` attribute
//!   (`bevel_weight_edge`) is not 0, its offset scaled by the weight; `VGROUP` every manifold
//!   edge whose two vertices weigh at least 0.5 in `vertex_group` (`invert_vertex_group`
//!   inverts); `NONE` every manifold edge.
//! - `affect='VERTICES'`: `WEIGHT` every vertex whose `vertex_weight` (`bevel_weight_vert`) is
//!   not 0 (offset scaled by it); otherwise every vertex weighing at least 0.5 in the vertex
//!   group (every vertex without one), its offset scaled by the group weight.
//!
//! `width` is the amount (`width_pct` is the same stored value, as in Blender); a zero amount
//! disables the modifier. A weight attribute of another type or domain is converted to a
//! temporary FLOAT attribute as Blender does (`lookup<float>`: domain adaptation in the source
//! type, then type conversion); QUATERNION, FLOAT4X4 and STRING attributes, which have no
//! conversion, are treated as missing (weight 0, WARNING). `harden_normals` writes custom normals as Blender does
//! (`bevel_harden_normals`, M5+M6 W0) and `profile_type='CUSTOM'` is a modifier error (no custom
//! profile curve yet).

use frap_bmesh::ops::bevel::{BevelIn, MeshBevelWeights, mesh_bevel_tagged};
use frap_bmesh::{BMesh, BmError, ElemFlag, FromMeshParams, ToMeshParams};
use frap_mesh::{AttrData, Attribute, Domain};
use frap_rna::{Report, ReportType};
use frap_scene::generated::modifier_settings::BevelSettings;
use frap_scene::id::slots;

use crate::prelude::*;

#[modifier(
    BEVEL,
    kind = Constructive,
    flags = ACCEPTS_MESH | SUPPORTS_EDITMODE | ENABLE_IN_EDITMODE | ACCEPTS_CVS,
    is_disabled = is_disabled,
    krate = crate
)]
fn eval(md: &Modifier, mesh: Mesh, cx: &mut dyn ModEvalCx) -> Result<Mesh, ModError> {
    let ModifierSettings::Bevel(s) = &md.settings else {
        return Ok(mesh);
    };
    let totcol = slots::slot_count(cx.main(), cx.object()) as i32;
    let out = modify_mesh(s, mesh, totcol)?;
    for w in out.warnings {
        cx.report(Report::new(
            ReportType::Warning,
            format!("Modifier '{}': {w}", md.name),
        ));
    }
    Ok(out.mesh)
}

/// The amount (`BevelModifierData::value`, which RNA exposes as `width` and as `width_pct`:
/// the settings store it once, in `width`).
fn value(s: &BevelSettings) -> f32 {
    s.width
}

/// `is_disabled`: a zero amount bevels nothing.
fn is_disabled(settings: &ModifierSettings) -> Option<&'static str> {
    match settings {
        ModifierSettings::Bevel(s) if value(s) == 0.0 => Some("Bevel: the amount is 0"),
        _ => None,
    }
}

/// The result of [`modify_mesh`].
#[derive(Debug)]
pub(crate) struct Beveled {
    /// The evaluated mesh.
    pub(crate) mesh: Mesh,
    /// What frap could not do as Blender does (reported as WARNINGs).
    pub(crate) warnings: Vec<String>,
}

/// The `bmesh.ops.bevel` identifier of a modifier enum item (`FSTR_NONE` → `NONE`,
/// `MITER_SHARP` → `SHARP`).
fn ident(s: &str) -> &'static str {
    match s {
        "OFFSET" => "OFFSET",
        "WIDTH" => "WIDTH",
        "DEPTH" => "DEPTH",
        "PERCENT" => "PERCENT",
        "ABSOLUTE" => "ABSOLUTE",
        "SUPERELLIPSE" => "SUPERELLIPSE",
        "CUSTOM" => "CUSTOM",
        "VERTICES" => "VERTICES",
        "EDGES" => "EDGES",
        "FSTR_NONE" | "NONE" => "NONE",
        "FSTR_NEW" | "NEW" => "NEW",
        "FSTR_AFFECTED" | "AFFECTED" => "AFFECTED",
        "FSTR_ALL" | "ALL" => "ALL",
        "MITER_SHARP" | "SHARP" => "SHARP",
        "MITER_PATCH" | "PATCH" => "PATCH",
        "MITER_ARC" | "ARC" => "ARC",
        "ADJ" => "ADJ",
        "CUTOFF" => "CUTOFF",
        _ => "",
    }
}

/// `ensure_weight_attribute_meta_data`: the name of a FLOAT attribute on `domain` holding the
/// weights of `name` (a converted temporary copy when the attribute has another type or
/// domain, flagged `true`), or `None` when there is no usable attribute. The conversion is
/// `attributes.lookup<float>(name, domain)` ([`Mesh::lookup_float`]): the domain is adapted in
/// the source type (a BOOLEAN edge is the AND of its vertices, a BOOLEAN vertex the OR of its
/// edges, integers are mixed with rounding), then the values are converted to float.
fn ensure_weight_attribute(
    mesh: &mut Mesh,
    name: &str,
    domain: Domain,
    warnings: &mut Vec<String>,
) -> Option<(String, bool)> {
    // `bke::allow_procedural_attribute_access`: internal (`.`-prefixed) names are refused.
    if name.is_empty() || name.starts_with('.') {
        return None;
    }
    let attr = mesh.attributes().get(name)?;
    if attr.domain == domain && matches!(attr.data, AttrData::Float(_)) {
        return Some((name.to_owned(), false));
    }
    let Some(weights) = mesh.lookup_float(name, domain).map(|w| w.into_owned()) else {
        warnings.push(format!(
            "the weight attribute \"{name}\" has a type that cannot be read as weights; it is \
             treated as missing"
        ));
        return None;
    };
    let new_name = mesh.unique_attribute_name(name);
    mesh.attributes_mut().insert(Attribute {
        name: new_name.clone(),
        domain,
        data: AttrData::Float(std::sync::Arc::new(weights)),
    });
    Some((new_name.to_string(), true))
}

/// `BKE_defvert_find_weight`.
fn defvert_weight(mesh: &Mesh, v: usize, group: usize) -> f32 {
    mesh.deform_verts()
        .and_then(|d| d.get(v))
        .and_then(|dv| dv.weights.iter().find(|w| w.0 as usize == group))
        .map_or(0.0, |w| w.1)
}

// Ported from blender/source/blender/modifiers/intern/MOD_bevel.cc (modify_mesh).
/// `modify_mesh` of the BEVEL modifier; `totcol` is the object's material slot count.
pub(crate) fn modify_mesh(
    s: &BevelSettings,
    mut mesh: Mesh,
    totcol: i32,
) -> Result<Beveled, ModError> {
    let mut warnings = Vec::new();
    if mesh.verts_num() == 0 {
        return Ok(Beveled { mesh, warnings });
    }
    if s.profile_type == "CUSTOM" {
        return Err(ModError::Error(
            "profile_type='CUSTOM' (a custom profile curve) is not supported by frap yet; use \
             'SUPERELLIPSE' with profile"
                .into(),
        ));
    }
    let threshold = libm::cosf(s.angle_limit + 0.000_000_175);
    let mat = s.material.clamp(-1, totcol - 1);
    let invert_vgroup = s.invert_vertex_group;
    let lim_weight = s.limit_method == "WEIGHT";
    let lim_vgroup = s.limit_method == "VGROUP";
    let lim_angle = s.limit_method == "ANGLE";
    let affect_vertices = s.affect == "VERTICES";

    let vert_weight =
        ensure_weight_attribute(&mut mesh, &s.vertex_weight, Domain::Point, &mut warnings);
    let edge_weight =
        ensure_weight_attribute(&mut mesh, &s.edge_weight, Domain::Edge, &mut warnings);

    let mut bm = BMesh::from_mesh(
        &mesh,
        &FromMeshParams {
            calc_face_normal: true,
            calc_vert_normal: true,
            use_shapekey: false,
            active_shapekey: 0,
        },
    );

    // `MOD_get_vgroup`: the group index on the mesh, and its weights when the mesh has any.
    let vgroup: Option<usize> = (lim_vgroup && !s.vertex_group.is_empty())
        .then(|| {
            mesh.vertex_group_names
                .iter()
                .position(|g| g == &s.vertex_group)
        })
        .flatten();
    let has_dvert = vgroup.is_some() && mesh.deform_verts().is_some();
    // `BKE_defvert_array_find_weight_safe`.
    let weight_safe = |v: usize| -> f32 {
        let Some(g) = vgroup else {
            return 1.0;
        };
        if !has_dvert {
            return if invert_vgroup { 1.0 } else { 0.0 };
        }
        let w = defvert_weight(&mesh, v, g);
        if invert_vgroup { 1.0 - w } else { w }
    };

    let bweight_vert = vert_weight.as_ref().and_then(|(n, _)| {
        bm.vdata
            .find(n)
            .filter(|&i| matches!(bm.vdata.layers[i].data, AttrData::Float(_)))
    });
    let bweight_edge = edge_weight.as_ref().and_then(|(n, _)| {
        bm.edata
            .find(n)
            .filter(|&i| matches!(bm.edata.layers[i].data, AttrData::Float(_)))
    });
    let float_at = |cd: &frap_bmesh::CustomData, layer: Option<usize>, slot: usize| -> f32 {
        layer.map_or(0.0, |li| match &cd.layers[li].data {
            AttrData::Float(c) => c.get(slot).copied().unwrap_or(0.0),
            _ => 0.0,
        })
    };

    if affect_vertices {
        for v in bm.vert_ids() {
            let skip = if lim_weight {
                float_at(&bm.vdata, bweight_vert, v.slot()) == 0.0
            } else {
                // Against 0.5 rather than 0: cascaded bevel modifiers interpolate weights for
                // new vertices, which could cause an unexpected "selection".
                weight_safe(v.slot()) < 0.5
            };
            if !skip {
                bm.vert_mut(v).flag.insert(ElemFlag::TAG);
            }
        }
    } else if lim_angle {
        for e in bm.edge_ids() {
            // An edge with two face users.
            let Some(l_a) = bm.edge(e).l else {
                continue;
            };
            let l_b = bm.lp(l_a).radial_next;
            if l_a == l_b || bm.lp(l_b).radial_next != l_a {
                continue;
            }
            let (na, nb) = (bm.face(bm.lp(l_a).f).no, bm.face(bm.lp(l_b).f).no);
            if na[0] * nb[0] + na[1] * nb[1] + na[2] * nb[2] < threshold {
                tag_edge(&mut bm, e);
            }
        }
    } else {
        for e in bm.edge_ids() {
            if !bm.edge_is_manifold(e) {
                continue;
            }
            if lim_weight {
                if float_at(&bm.edata, bweight_edge, e.slot()) == 0.0 {
                    continue;
                }
            } else {
                let [v1, v2] = bm.edge(e).v;
                if weight_safe(v1.slot()) < 0.5 || weight_safe(v2.slot()) < 0.5 {
                    continue;
                }
            }
            tag_edge(&mut bm, e);
        }
    }

    let vertex_group_weights = match vgroup {
        Some(g) if has_dvert => Some(
            (0..mesh.verts_num())
                .map(|v| defvert_weight(&mesh, v, g))
                .collect(),
        ),
        _ => None,
    };
    let input = BevelIn {
        geom: Vec::new(),
        offset: value(s),
        offset_type: ident(&s.offset_type),
        profile_type: ident(&s.profile_type),
        segments: s.segments,
        profile: s.profile,
        affect: ident(&s.affect),
        clamp_overlap: s.use_clamp_overlap,
        material: mat,
        loop_slide: s.loop_slide,
        mark_seam: s.mark_seam,
        mark_sharp: s.mark_sharp,
        harden_normals: s.harden_normals,
        face_strength_mode: ident(&s.face_strength_mode),
        miter_outer: ident(&s.miter_outer),
        miter_inner: ident(&s.miter_inner),
        spread: s.spread,
        custom_profile: None,
        vmesh_method: ident(&s.vmesh_method),
    };
    // The `--max-elements` pre-check (ARCHITECTURE §12), as `mesh.bevel` does: an absurd
    // request is refused at once instead of building geometry for minutes.
    let estimate = bevel_corners(&bm, u64::try_from(s.segments).unwrap_or(1), affect_vertices);
    if let Err(e) = frap_math::budget::reserve_corners(estimate, estimate) {
        // The payload `budget::check_corners` unwinds with: the stack makes it this
        // modifier's error (`stack::evaluate`), `object.modifier_apply` LIMIT_EXCEEDED.
        std::panic::resume_unwind(Box::new(e));
    }
    let weights = MeshBevelWeights {
        use_weights: lim_weight,
        bweight_vert,
        bweight_edge,
        vertex_group_weights,
    };
    mesh_bevel_tagged(&mut bm, &input, s.use_clamp_overlap, weights).map_err(|e| match e {
        BmError::Invalid(m) => ModError::Error(m),
        e => ModError::Error(e.to_string()),
    })?;

    let mut result = bm.to_mesh(&mesh, &ToMeshParams::default());
    for (name, converted) in [vert_weight, edge_weight].into_iter().flatten() {
        if converted {
            result.attributes_mut().remove(&name);
        }
    }
    Ok(Beveled {
        mesh: result,
        warnings,
    })
}

/// Face corners after beveling the tagged elements with `segments` (the estimate of
/// `frap-ops-mesh` `_growth::bevel_corners` over `BM_ELEM_TAG` instead of the selection): a
/// strip of `segments` quads along every tagged edge, a vertex patch of about
/// `valence · (segments / 2 + 1)²` quads at every beveled vertex, and `segments + 1` more
/// corners on each face corner there. `None`: the estimate overflowed.
fn bevel_corners(bm: &BMesh, segments: u64, affect_vertices: bool) -> Option<u64> {
    let tagged = |f: ElemFlag| f.contains(ElemFlag::TAG);
    let mut total = bm.loops_num() as u64;
    let seg = segments.max(1);
    let half = seg / 2 + 1;
    let patch = half.checked_mul(half)?.checked_mul(4)?;
    let per_vert = patch.checked_add(seg.checked_add(1)?.checked_mul(2)?)?;
    for v in bm.vert_ids() {
        let edges = bm.vert_edges(v);
        let beveled = if affect_vertices {
            tagged(bm.vert(v).flag)
        } else {
            edges.iter().any(|&e| tagged(bm.edge(e).flag))
        };
        if beveled {
            total = total.checked_add((edges.len() as u64).checked_mul(per_vert)?)?;
        }
    }
    if !affect_vertices {
        let te = bm
            .edge_ids()
            .into_iter()
            .filter(|&e| tagged(bm.edge(e).flag))
            .count() as u64;
        total = total.checked_add(te.checked_mul(seg)?.checked_mul(4)?)?;
    }
    Some(total)
}

/// Tags an edge and both its vertices (`BM_ELEM_TAG`).
fn tag_edge(bm: &mut BMesh, e: frap_bmesh::EdgeId) {
    bm.edge_mut(e).flag.insert(ElemFlag::TAG);
    let [a, b] = bm.edge(e).v;
    bm.vert_mut(a).flag.insert(ElemFlag::TAG);
    bm.vert_mut(b).flag.insert(ElemFlag::TAG);
}

#[cfg(test)]
mod tests;
