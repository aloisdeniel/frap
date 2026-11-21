// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_animation_pointer` import (WP 6.9, `import/pointer/**`): pointer channels on materials
//! (into the embedded node tree's action, `NT` slot), lights and cameras, ported from
//! io_scene_gltf2 5.2.41 (`imp/animation_pointer.py`, `imp/animation.py`,
//! `imp/animation_utils.py`, `imp/blender_gltf.py::dispatch_animation_pointer`), Apache-2.0,
//! The glTF-Blender-IO authors. Called by the animation importer (WP 6.7).
//!
//! The add-on reads pointers only when the file lists `KHR_animation_pointer` in
//! `extensionsUsed`. Per glTF animation (last to first, so the first one ends on top of the
//! NLA stacks), it creates **one** action named after the animation's track name
//! ([`track_names`]: the animation's name, or `Anim_<i>`, made unique), with one layer and one
//! keyframe strip; each animated datablock gets a slot of that action (`slots.new(id_type,
//! "Slot")`: `CASlot`, `LASlot`, `NTSlot`, …) holding its F-curves (`make_fcurve`: keys at
//! `time · fps`, LINEAR / STEP→CONSTANT / CUBICSPLINE→BEZIER with AUTO handles, the spline
//! values only). Every datablock is then stashed on an NLA track named after the animation
//! (`simulate_stash`: muted, locked, the strip over the action's frame range starting at the
//! scene's first frame), and the first animation is assigned back
//! (`restore_animation_on_object`).
//!
//! Targets, values and the add-on's quirks kept (see [`do_channel`]): a camera's `yfov`
//! becomes `lens` (with the camera's sensor height), `xmag`/`ymag` together one `ortho_scale`
//! (the larger magnification: the add-on's `max(x, y) * 2` repeats a 1-tuple, verified on
//! 5.2.4), only when both channels exist with the same keys;
//! a light's `intensity` becomes `energy` through the *directional* conversion for every type
//! (`asset['type']` is the glTF type, never `"SPOT"`/`"POINT"`), `spot.outerConeAngle`
//! `spot_size = 2·a`, `spot.innerConeAngle` `spot_blend = 1 − inner / outer` (with the outer
//! channel's values when its keys are the same); `range` is not imported. Material targets
//! are resolved in the imported node tree by [`material`].
//!
//! The animation importer of WP 6.7 shares the per-animation action with the node and weight
//! channels: it calls [`anim`] with its [`ActionCache`] for each animation after its own
//! channels, stashes [`ActionCache::needs_stash`] with [`simulate_stash`], and restores the
//! first animation of pointer data with [`restore`]. Until then [`create`] does all of it for
//! the pointer channels alone. [`dispatch`] classifies a channel, `/nodes/…` and `/meshes/…`
//! pointers included, for the node animation importer.

pub(crate) mod material;

use std::collections::BTreeMap;

use frap_anim::eval::recalc_handles;
use frap_anim::fcurve::keys;
use frap_anim::rna::anim_data::{AdtLoc, set as adt_set};
use frap_anim::{
    Action, ActionLayer, ActionStrip, AnimData, HandleType, Interpolation, KeyframeStrip, NlaStrip,
    NlaTrack,
};
use frap_gltf::ext::khr_animation_pointer::KhrAnimationPointer;
use frap_gltf::ext::khr_lights_punctual::{KhrLightsPunctual, KhrLightsPunctualNode, LightType};
use frap_gltf::json::{AnimationPath, Channel, Extension, Interpolation as GInterp};
use frap_rna::naming::unique_subname;
use frap_rna::{
    ActionId, CameraId, EmbeddedSlot, IdRef, LightId, MaterialId, ObjectId, ReportType, Root, Value,
};
use frap_scene::{ChangeLog, Main};
use smol_str::SmolStr;

use super::settings::ExportImportConvertLightingMode as Mode;
use super::{Gltf, ImportError};

/// `PBR_WATTS_TO_LUMENS`.
const PBR_WATTS_TO_LUMENS: f64 = 683.0;

/// Whether pointer channels are imported (the animation importer, WP 6.7, may rely on it).
pub(crate) const IMPLEMENTED: bool = true;

/// The parts of a glTF material whose `animations` dict a pointer is filed under
/// (`dispatch_animation_pointer`), in the order `BlenderAnimation.anim` imports them.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum MatPart {
    /// `mat.animations`: `emissiveFactor`, `alphaCutoff`.
    Material,
    /// `mat.normal_texture.animations`: `scale`.
    Normal,
    /// `mat.occlusion_texture.animations`: `strength`.
    Occlusion,
    /// `mat.pbr_metallic_roughness.animations`.
    Pbr,
    /// The `KHR_texture_transform` of texture `i` of [`material::TEXTURES`] (`offset` and
    /// `scale`; `TEXTURES.len()` for a texture the add-on does not import, whose dispatch
    /// fails).
    TextureTransform(usize),
    /// `mat.extensions[ext].animations`, by position in [`MATERIAL_EXTENSIONS`].
    Ext(usize),
}

/// The material extensions whose animations are imported, in `BlenderAnimation.anim`'s order.
pub(crate) const MATERIAL_EXTENSIONS: [&str; 10] = [
    "KHR_materials_emissive_strength",
    "KHR_materials_iridescence",
    "KHR_materials_volume",
    "KHR_materials_ior",
    "KHR_materials_transmission",
    "KHR_materials_clearcoat",
    "KHR_materials_sheen",
    "KHR_materials_specular",
    "KHR_materials_anisotropy",
    "KHR_materials_dispersion",
];

/// What a `pointer` channel animates (`BlenderGlTF.dispatch_animation_pointer`).
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Target {
    /// `/nodes/{i}/translation|rotation|scale|weights` (node animation, WP 6.7).
    Node {
        /// Node index.
        node: usize,
        /// `translation`, `rotation`, `scale` or `weights`.
        path: String,
    },
    /// `/meshes/{i}/weights`, broadcast to the nodes instancing the mesh (WP 6.7).
    MeshWeights {
        /// Mesh index.
        mesh: usize,
    },
    /// A camera property.
    Camera(usize),
    /// A `KHR_lights_punctual` light property.
    Light(usize),
    /// A material property.
    Material(usize, MatPart),
}

/// The pointer of a channel, when it is a `pointer` channel with a pointer string.
pub(crate) fn pointer_of(ch: &Channel) -> Option<String> {
    if ch.target.path != AnimationPath::Pointer {
        return None;
    }
    KhrAnimationPointer::of(&ch.target)
}

/// `dispatch_animation_pointer`: `None` for a channel that is not a pointer, or a pointer to
/// something the add-on does not import.
pub(crate) fn dispatch(ch: &Channel) -> Option<Target> {
    let p = pointer_of(ch)?;
    let tab = KhrAnimationPointer::split(&p);
    let n = tab.len();
    let idx = |i: usize| tab.get(i).and_then(|s| s.parse::<usize>().ok());
    let at = |i: usize| tab.get(i).copied().unwrap_or("");
    if n >= 4
        && at(1) == "nodes"
        && matches!(at(3), "translation" | "rotation" | "scale" | "weights")
    {
        return Some(Target::Node {
            node: idx(2)?,
            path: at(3).to_owned(),
        });
    }
    if n >= 4 && at(1) == "meshes" && at(3) == "weights" {
        return Some(Target::MeshWeights { mesh: idx(2)? });
    }
    if n == 5
        && at(1) == "cameras"
        && ((at(3) == "perspective" && matches!(at(4), "yfov" | "znear" | "zfar"))
            || (at(3) == "orthographic" && matches!(at(4), "xmag" | "ymag")))
    {
        return Some(Target::Camera(idx(2)?));
    }
    if n == 6
        && at(1) == "extensions"
        && at(2) == "KHR_lights_punctual"
        && at(3) == "lights"
        && matches!(
            at(5),
            "intensity" | "color" | "range" | "spot.outerConeAngle" | "spot.innerConeAngle"
        )
    {
        return Some(Target::Light(idx(4)?));
    }
    if at(1) != "materials" {
        return None;
    }
    let m = idx(2)?;
    let part = match (n, at(3), at(4)) {
        (4, "emissiveFactor" | "alphaCutoff", _) => MatPart::Material,
        (5, "normalTexture", "scale") => MatPart::Normal,
        (5, "occlusionTexture", "strength") => MatPart::Occlusion,
        (5, "pbrMetallicRoughness", "baseColorFactor" | "roughnessFactor" | "metallicFactor") => {
            MatPart::Pbr
        }
        // Only `scale` and `offset` are dispatched (a `rotation` channel is never imported).
        (7, tex, "extensions")
            if matches!(
                tex,
                "emissiveTexture" | "normalTexture" | "occlusionTexture"
            ) && at(5) == "KHR_texture_transform"
                && matches!(at(6), "scale" | "offset") =>
        {
            MatPart::TextureTransform(texture_index("", tex))
        }
        (8, "pbrMetallicRoughness", tex)
            if matches!(tex, "baseColorTexture" | "metallicRoughnessTexture")
                && at(5) == "extensions"
                && at(6) == "KHR_texture_transform"
                && matches!(at(7), "scale" | "offset") =>
        {
            MatPart::TextureTransform(texture_index("pbrMetallicRoughness", tex))
        }
        (9, "extensions", ext)
            if at(6) == "extensions"
                && at(7) == "KHR_texture_transform"
                && matches!(at(8), "scale" | "offset") =>
        {
            MatPart::TextureTransform(texture_index(ext, at(5)))
        }
        (_, _, _) if n >= 7 && at(n - 2) == "KHR_texture_transform" => return None,
        (6 | 7, "extensions", ext) => {
            let member = at(5);
            let ok = match ext {
                "KHR_materials_emissive_strength" => member == "emissiveStrength" && n == 6,
                "KHR_materials_volume" => {
                    matches!(
                        member,
                        "thicknessFactor" | "attenuationDistance" | "attenuationColor"
                    )
                }
                "KHR_materials_ior" => member == "ior",
                "KHR_materials_transmission" => member == "transmissionFactor",
                "KHR_materials_clearcoat" => {
                    (n == 6 && matches!(member, "clearcoatFactor" | "clearcoatRoughnessFactor"))
                        || (n == 7 && member == "clearcoatNormalTexture" && at(6) == "scale")
                }
                "KHR_materials_sheen" => {
                    matches!(member, "sheenColorFactor" | "sheenRoughnessFactor")
                }
                "KHR_materials_specular" => {
                    matches!(member, "specularFactor" | "specularColorFactor")
                }
                "KHR_materials_anisotropy" => {
                    matches!(member, "anisotropyStrength" | "anisotropyRotation")
                }
                "KHR_materials_dispersion" => member == "dispersion",
                "KHR_materials_iridescence" => matches!(
                    member,
                    "iridescenceFactor"
                        | "iridescenceIor"
                        | "iridescenceThicknessMinimum"
                        | "iridescenceThicknessMaximum"
                ),
                _ => false,
            };
            if !ok || (n == 7 && ext != "KHR_materials_clearcoat") {
                return None;
            }
            MatPart::Ext(MATERIAL_EXTENSIONS.iter().position(|e| *e == ext)?)
        }
        _ => return None,
    };
    Some(Target::Material(m, part))
}

/// The index of `(owner, member)` in [`material::TEXTURES`] (`TEXTURES.len()` when absent).
fn texture_index(owner: &str, member: &str) -> usize {
    material::TEXTURES
        .iter()
        .position(|&(o, m)| o == owner && m == member)
        .unwrap_or(material::TEXTURES.len())
}

/// Whether the pointer import handles `ch` (a camera, light or material pointer).
pub(crate) fn handles(ch: &Channel) -> bool {
    matches!(
        dispatch(ch),
        Some(Target::Camera(_) | Target::Light(_) | Target::Material(..))
    )
}

/// `find_unused_name(haystack, desired_name)` (`blender/com/gltf2_blender_utils.py`): `a`,
/// then `a.001`, `a.002`, …, within 63 UTF-8 bytes.
pub(crate) fn find_unused_name(taken: &[String], desired: &str) -> String {
    let mut stem: String = desired.chars().take(63).collect();
    let mut suffix = String::new();
    let mut cntr = 1;
    loop {
        let name = format!("{stem}{suffix}");
        if name.len() > 63 {
            stem.pop();
            continue;
        }
        if !taken.contains(&name) {
            return name;
        }
        suffix = format!(".{cntr:03}");
        cntr += 1;
    }
}

/// `anim.track_name` of every animation (`BlenderGlTF.set_convert_functions` /
/// `create`: pair-wise unique names).
pub(crate) fn track_names(cx: &Gltf) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for (i, a) in cx.doc.animations.iter().enumerate() {
        let desired = super::truthy_name(&a.name).unwrap_or_else(|| format!("Anim_{i}"));
        let n = find_unused_name(&names, &desired);
        names.push(n);
    }
    names
}

/// The action and slots one glTF animation creates (`gltf.action_cache[anim_idx]` and
/// `gltf.needs_stash`).
#[derive(Clone, Debug, Default)]
pub(crate) struct ActionCache {
    /// The animation's action, once created.
    pub action: Option<IdRef<ActionId>>,
    /// `object_slots[name][target_id_type]` → slot handle.
    pub slots: Vec<((Option<String>, String), i32)>,
    /// `(datablock, action, slot)` to stash, in creation order.
    pub needs_stash: Vec<(Root, IdRef<ActionId>, i32)>,
}

/// The Blender datablocks the importer created for the glTF cameras, lights and materials
/// (`pycamera.blender_object_data`, `pylight['blender_object_data']`,
/// `pymaterial.blender_nodetree`): the last one created for each.
#[derive(Clone, Debug, Default)]
pub(crate) struct Created {
    cameras: BTreeMap<usize, IdRef<CameraId>>,
    lights: BTreeMap<usize, IdRef<LightId>>,
    materials: BTreeMap<usize, IdRef<MaterialId>>,
}

impl Created {
    pub(crate) fn of(cx: &Gltf, main: &Main) -> Created {
        let mut c = Created::default();
        let mut vnode_of: BTreeMap<IdRef<ObjectId>, usize> = BTreeMap::new();
        for (i, v) in cx.vtree.nodes.iter().enumerate() {
            if let Some(o) = v.blender_object {
                vnode_of.insert(o, i);
            }
        }
        for &ob_ref in &cx.objects {
            let (Some(&vi), Some(ob)) = (vnode_of.get(&ob_ref), main.objects.get(ob_ref)) else {
                continue;
            };
            let v = &cx.vtree.nodes[vi];
            if let (Some(ni), Some(cam)) = (v.camera_node_idx, ob.data.and_then(|d| d.typed()))
                && let Some(ci) = cx.doc.nodes.get(ni).and_then(|n| n.camera)
            {
                c.cameras.insert(ci as usize, cam);
            }
            if let (Some(ni), Some(la)) = (v.light_node_idx, ob.data.and_then(|d| d.typed()))
                && let Ok(Some(ext)) = cx.doc.nodes[ni].extensions.get::<KhrLightsPunctualNode>()
            {
                c.lights.insert(ext.light as usize, la);
            }
        }
        for (&(mi, vertex_color), &r) in &cx.materials {
            // Without vertex colours first, so a material imported both ways keeps that one
            // only when it is the only one (the add-on keeps the last created).
            if vertex_color || !c.materials.contains_key(&mi) {
                c.materials.insert(mi, r);
            }
        }
        c
    }
}

/// The per-asset state the add-on keeps across channels (`multiple_channels`,
/// `multiple_channels_mag`).
#[derive(Clone, Debug, Default)]
pub(crate) struct PointerState {
    /// Orthographic cameras: `(anim, channel)` of their `xmag` / `ymag` channels, emptied once
    /// used.
    mag: BTreeMap<usize, BTreeMap<String, (usize, usize)>>,
    /// Spot lights: `(anim, channel)` of their cone channels.
    cones: BTreeMap<usize, BTreeMap<String, (usize, usize)>>,
    /// Texture transforms (material, texture of [`material::TEXTURES`]): `(anim, channel)` of
    /// their `offset` / `scale` channels, the last one of each over every animation
    /// (`texture()`'s `multiple_channels`).
    tex: BTreeMap<(usize, usize), BTreeMap<String, (usize, usize)>>,
}

impl PointerState {
    /// `BlenderCamera.create` / `BlenderLight.create_spot` record every animation's channels.
    pub(crate) fn of(cx: &Gltf) -> PointerState {
        let mut s = PointerState::default();
        for (ai, a) in cx.doc.animations.iter().enumerate() {
            for (ci, ch) in a.channels.iter().enumerate() {
                let Some(p) = pointer_of(ch) else { continue };
                let tab = KhrAnimationPointer::split(&p);
                match dispatch(ch) {
                    Some(Target::Camera(c))
                        if tab.get(3) == Some(&"orthographic")
                            && cx
                                .doc
                                .cameras
                                .get(c)
                                .is_some_and(|cam| cam.orthographic.is_some()) =>
                    {
                        s.mag
                            .entry(c)
                            .or_default()
                            .insert(tab[4].to_owned(), (ai, ci));
                    }
                    Some(Target::Light(l)) if tab[5].starts_with("spot.") => {
                        s.cones
                            .entry(l)
                            .or_default()
                            .insert(tab[5].to_owned(), (ai, ci));
                    }
                    Some(Target::Material(m, MatPart::TextureTransform(t))) => {
                        s.tex
                            .entry((m, t))
                            .or_default()
                            .insert(tab[tab.len() - 1].to_owned(), (ai, ci));
                    }
                    _ => {}
                }
            }
        }
        s
    }
}

/// Keys and values of a channel's sampler (`values[1::3]` for CUBICSPLINE).
/// A sampler's key times, values (one element per key) and interpolation.
type SamplerData = (Vec<f32>, Vec<Vec<f32>>, GInterp);

fn sampler_data(cx: &Gltf, anim: usize, ch: usize) -> Result<SamplerData, ImportError> {
    let a = &cx.doc.animations[anim];
    let c = &a.channels[ch];
    let s = a.samplers.get(c.sampler as usize).ok_or_else(|| {
        ImportError::Data(format!("animation {anim}: sampler {} missing", c.sampler))
    })?;
    let keys = cx.accessor(s.input)?.to_f32();
    let out = cx.accessor(s.output)?;
    let n = out.type_.components().max(1);
    let mut values: Vec<Vec<f32>> = out.to_f32().chunks_exact(n).map(<[f32]>::to_vec).collect();
    let interp = s.interpolation_or_default();
    if interp == GInterp::CubicSpline {
        values = values.into_iter().skip(1).step_by(3).collect();
    }
    Ok((keys, values, interp))
}

/// Why channel `ci` of animation `ai` cannot be imported as the F-curves `curves` (`None` when
/// it can): an output accessor whose element type does not match the target (`expected`
/// components), an output count that is not the input count (three times it for
/// CUBICSPLINE), or a key or converted value that is not finite (`yfov` 0 → infinite lens,
/// `outerConeAngle` 0 → NaN `spot_blend`). The add-on raises on these and imports nothing for
/// the file; frap skips the channel with a WARNING instead of keying corrupt values.
fn invalid_channel(
    cx: &Gltf,
    ai: usize,
    ci: usize,
    expected: usize,
    curves: &Curves,
    keys: &[f32],
) -> Option<String> {
    let a = &cx.doc.animations[ai];
    let s = a.samplers.get(a.channels[ci].sampler as usize)?;
    let input = cx.doc.accessors.get(s.input as usize)?;
    let output = cx.doc.accessors.get(s.output as usize)?;
    let comps = output.type_.components();
    if comps != expected {
        return Some(format!(
            "its output has {comps} component(s), the target needs {expected}"
        ));
    }
    let per_key = if s.interpolation_or_default() == GInterp::CubicSpline {
        3
    } else {
        1
    };
    if output.count != input.count * per_key {
        return Some(format!(
            "its sampler has {} output(s) for {} input(s)",
            output.count, input.count
        ));
    }
    let finite = keys.iter().all(|k| k.is_finite())
        && curves
            .values
            .iter()
            .flat_map(|v| v.iter().take(curves.components))
            .all(|v| v.is_finite())
        && curves
            .alpha
            .as_ref()
            .is_none_or(|(_, a)| a.iter().all(|v| v.is_finite()));
    (!finite).then(|| "a key or converted value is not finite".to_owned())
}

/// The F-curves one channel becomes.
struct Curves {
    root: Root,
    id_type: &'static str,
    slot_name: Option<String>,
    path: String,
    components: usize,
    group: &'static str,
    values: Vec<Vec<f32>>,
    /// The alpha F-curve of a base colour: its path and the alpha values.
    alpha: Option<(String, Vec<f32>)>,
}

/// `get_or_create_action_and_slot` of the pointer importer.
fn action_and_slot(
    main: &mut Main,
    log: &mut ChangeLog,
    cache: &mut ActionCache,
    track_name: &str,
    root: Root,
    id_type: &str,
    name: Option<String>,
) -> (IdRef<ActionId>, i32) {
    let act_ref = *cache.action.get_or_insert_with(|| {
        let mut a = Action::new("");
        a.layers.push(ActionLayer::new("layer0"));
        a.layers[0]
            .strips
            .push(ActionStrip::Keyframe(KeyframeStrip::default()));
        main.actions.insert(track_name, a, log)
    });
    let key = (name, id_type.to_owned());
    if let Some((_, h)) = cache.slots.iter().find(|(k, _)| *k == key) {
        return (act_ref, *h);
    }
    let handle = match main.actions.get_mut(act_ref, log) {
        Some(act) => {
            let h = act.slot_add_for_id_type(id_type, "Slot");
            act.channelbag_ensure(h);
            h
        }
        None => 0,
    };
    cache.slots.push((key, handle));
    cache.needs_stash.push((root, act_ref, handle));
    (act_ref, handle)
}

/// `make_fcurve`.
#[allow(clippy::too_many_arguments)]
fn make_fcurve(
    main: &mut Main,
    log: &mut ChangeLog,
    action: IdRef<ActionId>,
    slot: i32,
    times: &[f32],
    values: &[f32],
    path: &str,
    index: i32,
    group: &str,
    interp: GInterp,
) {
    let Some(act) = main.actions.get_mut(action, log) else {
        return;
    };
    let bag = act.channelbag_ensure(slot);
    // `fcurves.new` raises on an existing curve: the add-on gives up on that channel.
    let Some(i) = bag.fcurve_create_unique(path, index, None) else {
        return;
    };
    if !group.is_empty() {
        let g = bag.group_ensure(group);
        bag.fcurve_assign_to_group(i, g);
    }
    let Some(i) = bag.fcurve_find(path, index) else {
        return;
    };
    let fc = &mut bag.fcurves[i];
    keys::add_zeroed(fc, times.len());
    let ipo = match interp {
        GInterp::CubicSpline => Interpolation::Bezier,
        GInterp::Step => Interpolation::Constant,
        GInterp::Linear => Interpolation::Linear,
    };
    for (k, (&t, &v)) in fc.keyframe_points.iter_mut().zip(times.iter().zip(values)) {
        k.co = [t, v];
        k.interpolation = ipo;
        if interp == GInterp::CubicSpline {
            k.handle_left_type = HandleType::Auto;
            k.handle_right_type = HandleType::Auto;
        }
    }
    keys::update(fc);
    recalc_handles(fc);
}

/// `BlenderPointerAnim.do_channel` for channel `ci` of animation `ai` on `target`: the F-curves
/// to create (`None` with a WARNING for an unsupported target).
#[allow(clippy::too_many_lines)]
fn do_channel(
    cx: &mut Gltf,
    main: &Main,
    created: &Created,
    state: &mut PointerState,
    ai: usize,
    ci: usize,
    target: &Target,
) -> Result<Option<(Curves, Vec<f32>, GInterp)>, ImportError> {
    let pointer = pointer_of(&cx.doc.animations[ai].channels[ci]).unwrap_or_default();
    let tab: Vec<&str> = KhrAnimationPointer::split(&pointer);
    let (keys, mut values, interp) = sampler_data(cx, ai, ci)?;
    let scalar = |values: &[Vec<f32>]| -> Vec<Vec<f32>> {
        values
            .iter()
            .map(|v| vec![v.first().copied().unwrap_or(0.0)])
            .collect()
    };
    let curves = match *target {
        Target::Camera(c) => {
            let Some(&cam_ref) = created.cameras.get(&c) else {
                return Ok(None);
            };
            let cam = main.cameras.get(cam_ref);
            let name = cx
                .doc
                .cameras
                .get(c)
                .and_then(|p| super::truthy_name(&p.name))
                .unwrap_or_else(|| "Camera".into());
            let (path, values) = match (tab[3], tab[4]) {
                ("perspective", "znear") => ("clip_start".to_owned(), scalar(&values)),
                ("perspective", "zfar") => ("clip_end".to_owned(), scalar(&values)),
                ("perspective", "yfov") => {
                    let sensor = f64::from(cam.map_or(24.0, |c| c.fields.sensor_height));
                    let v = values
                        .iter()
                        .map(|v| {
                            let fov = f64::from(v.first().copied().unwrap_or(0.0));
                            vec![((sensor / 2.0) / (fov * 0.5).tan()) as f32]
                        })
                        .collect();
                    ("lens".to_owned(), v)
                }
                (_, mag) => {
                    // `xmag` + `ymag` → one `ortho_scale` (only when both channels exist and
                    // share their keys; the record is emptied after the first one).
                    let rec = state.mag.get(&c).cloned().unwrap_or_default();
                    if rec.is_empty() {
                        cx.report(
                            ReportType::Warning,
                            format!("Unsupported animation target: {tab:?}"),
                        );
                        return Ok(None);
                    }
                    let other = if mag == "xmag" { "ymag" } else { "xmag" };
                    let (Some(&(ax, cx_)), Some(&(ay, cy))) = (rec.get("xmag"), rec.get("ymag"))
                    else {
                        let _ = other;
                        state.mag.insert(c, BTreeMap::new());
                        cx.report(
                            ReportType::Warning,
                            format!("Unsupported animation target: {tab:?}"),
                        );
                        return Ok(None);
                    };
                    let (kx, vx, _) = sampler_data(cx, ax, cx_)?;
                    let (ky, vy, _) = sampler_data(cx, ay, cy)?;
                    state.mag.insert(c, BTreeMap::new());
                    if kx != ky {
                        cx.report(
                            ReportType::Warning,
                            format!("Unsupported animation target: {tab:?}"),
                        );
                        return Ok(None);
                    }
                    let v = vx
                        .iter()
                        .zip(&vy)
                        // `max(xmag_values[idx], ymag_values[idx]) * 2` on 1-tuples repeats
                        // the tuple: the F-curve gets the larger magnification itself.
                        .map(|(x, y)| vec![x[0].max(y[0])])
                        .collect();
                    ("ortho_scale".to_owned(), v)
                }
            };
            Curves {
                root: Root::Id(cam_ref.any()),
                id_type: "CAMERA",
                slot_name: Some(name),
                path,
                components: 1,
                group: "Camera",
                values,
                alpha: None,
            }
        }
        Target::Light(l) => {
            let Some(&la_ref) = created.lights.get(&l) else {
                return Ok(None);
            };
            let lights = cx
                .doc
                .extensions
                .get::<KhrLightsPunctual>()
                .ok()
                .flatten()
                .unwrap_or_default();
            let Some(pylight) = lights.lights.get(l) else {
                return Ok(None);
            };
            let name = pylight.name.clone().unwrap_or_else(|| {
                match pylight.type_ {
                    LightType::Directional => "Sun",
                    LightType::Point => "Point",
                    LightType::Spot => "Spot",
                }
                .into()
            });
            let mode = cx.settings.export_import_convert_lighting_mode;
            let (path, components, values) = match tab[5] {
                "color" => ("color".to_owned(), 3, values),
                "intensity" => {
                    // `calc_energy_directional` for every type (see the module documentation).
                    let v = values
                        .iter()
                        .map(|v| {
                            let i = f64::from(v.first().copied().unwrap_or(0.0));
                            vec![match mode {
                                Mode::Spec => (i / PBR_WATTS_TO_LUMENS) as f32,
                                Mode::Compat | Mode::Raw => i as f32,
                            }]
                        })
                        .collect();
                    ("energy".to_owned(), 1, v)
                }
                "spot.outerConeAngle" => (
                    "spot_size".to_owned(),
                    1,
                    values
                        .iter()
                        .map(|v| vec![v.first().copied().unwrap_or(0.0) * 2.0])
                        .collect(),
                ),
                "spot.innerConeAngle" => {
                    let rec = state.cones.get(&l).cloned().unwrap_or_default();
                    let (outer_keys, outer_values) = match rec.get("spot.outerConeAngle") {
                        Some(&(a, c)) => {
                            let (k, v, _) = sampler_data(cx, a, c)?;
                            (k, v)
                        }
                        None => {
                            let o = pylight
                                .spot
                                .as_ref()
                                .map_or(std::f64::consts::FRAC_PI_4, |s| {
                                    s.outer_cone_angle_or_default()
                                });
                            (keys.clone(), vec![vec![o as f32]; keys.len()])
                        }
                    };
                    if keys == outer_keys {
                        values = values
                            .iter()
                            .zip(&outer_values)
                            .map(|(i, o)| vec![1.0 - i[0] / o[0]])
                            .collect();
                    }
                    ("spot_blend".to_owned(), 1, values)
                }
                _ => {
                    cx.report(
                        ReportType::Warning,
                        format!("Unsupported animation target: {tab:?}"),
                    );
                    return Ok(None);
                }
            };
            Curves {
                root: Root::Id(la_ref.any()),
                id_type: "LIGHT",
                slot_name: Some(name),
                path,
                components,
                group: "Light",
                values,
                alpha: None,
            }
        }
        Target::Material(m, _) => {
            let Some(&mat_ref) = created.materials.get(&m) else {
                return Ok(None);
            };
            let Some(mat) = main.materials.get(mat_ref) else {
                return Ok(None);
            };
            let gm_name = cx.doc.materials[m].name.clone();
            let is_unlit = cx.doc.materials[m]
                .extensions
                .contains("KHR_materials_unlit");
            let resolved = match *target {
                Target::Material(_, MatPart::TextureTransform(t)) => {
                    // `is_unlit` is passed for the core textures only.
                    let unlit = is_unlit && t < 5;
                    material::texture_transform(main, mat, t, tab[tab.len() - 1], unlit)
                }
                _ => material::resolve(main, mat, &tab, is_unlit),
            };
            let Some(t) = resolved else {
                cx.report(
                    ReportType::Warning,
                    format!("Unsupported animation target: {tab:?}"),
                );
                return Ok(None);
            };
            let values: Vec<Vec<f32>> = match t.convert {
                material::Convert::None => values,
                material::Convert::Inverse => values.iter().map(|v| vec![1.0 / v[0]]).collect(),
                material::Convert::Half => values.iter().map(|v| vec![v[0] / 2.0]).collect(),
            };
            let values = match *target {
                Target::Material(m, MatPart::TextureTransform(ti))
                    if tab[tab.len() - 1] == "offset" =>
                {
                    offset_to_location(cx, state, m, ti, &keys, values)?
                }
                _ => values,
            };
            let is_base_color =
                tab.len() == 5 && tab[3] == "pbrMetallicRoughness" && tab[4] == "baseColorFactor";
            let alpha_path = if is_base_color {
                match material::alpha_path(main, mat, is_unlit) {
                    Ok(p) => p,
                    Err(()) => {
                        // `alpha_socket` is unbound in the add-on (UnboundLocalError): the
                        // colour F-curves are made, then the import fails.
                        cx.report(
                            ReportType::Warning,
                            format!(
                                "glTF import: KHR_animation_pointer channel {pointer}: the \
                                 material has no alpha socket; only the colour is imported"
                            ),
                        );
                        None
                    }
                }
            } else {
                None
            };
            let alpha = alpha_path.map(|p| {
                (
                    p,
                    values
                        .iter()
                        .map(|v| v.get(3).copied().unwrap_or(0.0))
                        .collect(),
                )
            });
            Curves {
                root: Root::Embedded {
                    owner: mat_ref.any(),
                    slot: EmbeddedSlot::NodeTree,
                },
                id_type: "NODETREE",
                slot_name: gm_name,
                path: t.path,
                components: t.components,
                group: t.group,
                values,
                alpha,
            }
        }
        _ => return Ok(None),
    };
    Ok(Some((curves, keys, interp)))
}

/// `do_channel`'s offset conversion: `texture_transform_gltf_to_blender` of each key with
/// the texture's static rotation (no `rotation` channel is ever recorded) and the scale of the
/// texture's recorded `scale` channel, else its static scale, when the keys are the same
/// (otherwise the glTF offsets are written as they are). The recorded scale values are read
/// unsliced, as the add-on reads them.
fn offset_to_location(
    cx: &Gltf,
    state: &PointerState,
    m: usize,
    t: usize,
    keys: &[f32],
    values: Vec<Vec<f32>>,
) -> Result<Vec<Vec<f32>>, ImportError> {
    let tt = material::texture_transform_of(&cx.doc.materials[m], t).unwrap_or_default();
    let num = |v: &serde_json::Value| v.as_f64();
    let rotation = tt.get("rotation").and_then(num).unwrap_or(0.0);
    let static_scale = tt
        .get("scale")
        .and_then(|s| s.as_array())
        .map(|a| a.iter().filter_map(num).collect::<Vec<f64>>())
        .unwrap_or_else(|| vec![1.0, 1.0]);
    let (keys_scale, values_scale): (Vec<f32>, Vec<Vec<f64>>) =
        match state.tex.get(&(m, t)).and_then(|r| r.get("scale")) {
            Some(&(a, c)) => {
                let an = &cx.doc.animations[a];
                let s = &an.samplers[an.channels[c].sampler as usize];
                let k = cx.accessor(s.input)?.to_f32();
                let out = cx.accessor(s.output)?;
                let n = out.type_.components().max(1);
                let v = out
                    .to_f32()
                    .chunks_exact(n)
                    .map(|c| c.iter().map(|&x| f64::from(x)).collect())
                    .collect();
                (k, v)
            }
            None => (keys.to_vec(), vec![static_scale; keys.len()]),
        };
    if keys != keys_scale.as_slice() {
        return Ok(values);
    }
    Ok(values
        .iter()
        .enumerate()
        .map(|(i, o)| {
            let s1 = values_scale
                .get(i)
                .and_then(|s| s.get(1).copied())
                .unwrap_or(1.0);
            let (o0, o1) = (
                f64::from(o.first().copied().unwrap_or(0.0)),
                f64::from(o.get(1).copied().unwrap_or(0.0)),
            );
            vec![
                (o0 + s1 * rotation.sin()) as f32,
                (1.0 - o1 - s1 * rotation.cos()) as f32,
            ]
        })
        .collect())
}

/// What the pointer import keeps across animations: the created datablocks and the add-on's
/// per-asset channel records.
#[derive(Clone, Debug, Default)]
pub(crate) struct ImportState {
    /// The datablocks of the glTF cameras, lights and materials.
    pub created: Created,
    /// `multiple_channels` / `multiple_channels_mag`.
    pub records: PointerState,
}

impl ImportState {
    /// The state after the scenes were created.
    pub(crate) fn of(cx: &Gltf, main: &Main) -> ImportState {
        ImportState {
            created: Created::of(cx, main),
            records: PointerState::of(cx),
        }
    }
}

/// `BlenderPointerAnim.anim` for every camera, light and material animated by animation `ai`
/// (in the add-on's order), into `cache`.
pub(crate) fn anim(
    cx: &mut Gltf,
    main: &mut Main,
    log: &mut ChangeLog,
    ai: usize,
    track_name: &str,
    st: &mut ImportState,
    cache: &mut ActionCache,
) -> Result<(), ImportError> {
    let created = &st.created;
    let state = &mut st.records;
    if !cx
        .doc
        .extensions_used
        .iter()
        .any(|e| e == KhrAnimationPointer::NAME)
    {
        return Ok(());
    }
    // Channels per target, in channel order.
    let mut cams: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    let mut lights: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    let mut mats: BTreeMap<(usize, MatPart), Vec<usize>> = BTreeMap::new();
    for (ci, ch) in cx.doc.animations[ai].channels.iter().enumerate() {
        match dispatch(ch) {
            Some(Target::Camera(c)) => cams.entry(c).or_default().push(ci),
            Some(Target::Light(l)) => lights.entry(l).or_default().push(ci),
            Some(Target::Material(m, part)) => mats.entry((m, part)).or_default().push(ci),
            _ => {}
        }
    }
    let mut work: Vec<(usize, Target)> = Vec::new();
    for (c, chs) in cams {
        work.extend(chs.into_iter().map(|ci| (ci, Target::Camera(c))));
    }
    for (l, chs) in lights {
        work.extend(chs.into_iter().map(|ci| (ci, Target::Light(l))));
    }
    for ((m, part), chs) in mats {
        // `if len(mat.blender_material) == 0: continue` (the material is not used).
        if !created.materials.contains_key(&m) {
            continue;
        }
        // `BlenderAnimation.anim` reads the part's `animations` only when the material has it
        // (the add-on's dispatch fails on a pointer into a missing part).
        let gm = &cx.doc.materials[m];
        let present = match &part {
            MatPart::Normal => gm.normal_texture.is_some(),
            MatPart::Occlusion => gm.occlusion_texture.is_some(),
            MatPart::Pbr => gm.pbr_metallic_roughness.is_some(),
            MatPart::Ext(i) => gm.extensions.contains(MATERIAL_EXTENSIONS[*i]),
            MatPart::TextureTransform(t) => material::texture_transform_of(gm, *t).is_some(),
            MatPart::Material => true,
        };
        if !present {
            cx.warn_once(
                "pointer_missing_part",
                "glTF import: KHR_animation_pointer channels into a material part the file does not define were skipped",
            );
            continue;
        }
        work.extend(
            chs.into_iter()
                .map(|ci| (ci, Target::Material(m, part.clone()))),
        );
    }
    let fps = main
        .scenes
        .get(cx.scene)
        .map_or(24.0, |s| s.render.fps as f32);
    for (ci, target) in work {
        let Some((curves, keys, interp)) = do_channel(cx, main, created, state, ai, ci, &target)?
        else {
            continue;
        };
        let pointer = pointer_of(&cx.doc.animations[ai].channels[ci]).unwrap_or_default();
        // A base colour is RGBA in the file (the alpha goes to its own F-curve).
        let expected = if pointer.ends_with("/baseColorFactor") {
            4
        } else {
            curves.components
        };
        if let Some(why) = invalid_channel(cx, ai, ci, expected, &curves, &keys) {
            cx.report(
                ReportType::Warning,
                format!("glTF import: KHR_animation_pointer channel {pointer} skipped: {why}"),
            );
            continue;
        }
        let (action, slot) = action_and_slot(
            main,
            log,
            cache,
            track_name,
            curves.root,
            curves.id_type,
            curves.slot_name.clone(),
        );
        let times: Vec<f32> = keys.iter().map(|t| t * fps).collect();
        for i in 0..curves.components {
            let v: Vec<f32> = curves
                .values
                .iter()
                .map(|v| v.get(i).copied().unwrap_or(0.0))
                .collect();
            make_fcurve(
                main,
                log,
                action,
                slot,
                &times,
                &v,
                &curves.path,
                i as i32,
                curves.group,
                interp,
            );
        }
        if let Some((path, alpha)) = &curves.alpha {
            make_fcurve(
                main,
                log,
                action,
                slot,
                &times,
                alpha,
                path,
                0,
                curves.group,
                interp,
            );
        }
    }
    Ok(())
}

/// The `AnimData` of a stash target, created when missing (`animation_data_create`).
fn adt_mut<'m>(
    main: &'m mut Main,
    log: &mut ChangeLog,
    root: Root,
) -> Option<&'m mut Option<AnimData>> {
    match root {
        Root::Id(id) => {
            if let Some(r) = id.typed::<CameraId>() {
                return main.cameras.get_mut(r, log).map(|c| &mut c.animation_data);
            }
            if let Some(r) = id.typed::<LightId>() {
                return main.lights.get_mut(r, log).map(|l| &mut l.animation_data);
            }
            if let Some(r) = id.typed::<MaterialId>() {
                return main
                    .materials
                    .get_mut(r, log)
                    .map(|m| &mut m.animation_data);
            }
            None
        }
        Root::Embedded {
            owner,
            slot: EmbeddedSlot::NodeTree,
        } => owner
            .typed::<MaterialId>()
            .and_then(|r| main.materials.get_mut(r, log))
            .map(|m| &mut m.node_tree.animation_data),
        _ => None,
    }
}

/// The name of an animated datablock and its ID type (for the strip's slot auto-assignment).
fn id_name_type(main: &Main, root: Root) -> (String, &'static str) {
    match root {
        Root::Id(id) => (
            main.header(id)
                .map(|h| format!("{}{}", id.code.as_str(), h.name))
                .unwrap_or_default(),
            frap_anim::action::id_type_of_prefix(id.code.as_str()),
        ),
        _ => (
            format!("NT{}", frap_scene::id::SHADER_NODETREE_NAME),
            "NODETREE",
        ),
    }
}

/// `sizeof(NlaTrack::name) - 1`
const NAME_MAX: usize = 63;

/// `simulate_stash(data, track_name, action, slot)`: a new muted, locked NLA track named
/// `track_name` on top of `root`'s stack, holding a strip of `action` (named after it) at the
/// scene's first frame, its slot `slot`, over the action's frame range.
pub(crate) fn simulate_stash(
    main: &mut Main,
    log: &mut ChangeLog,
    root: Root,
    track_name: &str,
    action: IdRef<ActionId>,
    slot: i32,
    start_frame: i32,
) {
    let (id_name, id_type) = id_name_type(main, root);
    let Some(act) = main.actions.get(action) else {
        return;
    };
    let action_name = act.header.name.clone();
    let range = act.frame_range_get();
    // `strips.new(action.name, start, action)`: `BKE_nlastrip_new` (the slot auto-assigned for
    // the datablock, the length of its keys), moved to `start`.
    let auto =
        act.slot_for_autoassign(id_type, &id_name, "")
            .or_else(|| match act.slots.as_slice() {
                [only] if only.is_suitable_for(id_type) => Some(only.handle),
                _ => None,
            });
    let auto_range = auto.map_or([0.0, 0.0], |h| slot_range(act, h));
    let mut strip = NlaStrip::new_clip("", Some(action), auto_range[0], auto_range);
    strip.use_sync_length = !act.use_frame_range;
    strip.use_animated_time_cyclic = act.use_cyclic && act.use_frame_range;
    strip.frame_end += start_frame as f32 - strip.frame_start;
    strip.frame_start = start_frame as f32;
    // `_strip.action_slot = slot`
    let ident = act.slot(slot).map(|s| s.identifier.clone());
    if let Some(a) = main.actions.get_mut(action, log) {
        a.slot_setup_for_id(slot, id_type);
    }
    let ident = main
        .actions
        .get(action)
        .and_then(|a| a.slot(slot))
        .map(|s| s.identifier.clone())
        .or(ident)
        .unwrap_or_default();
    strip.action_slot_handle = slot;
    strip.last_slot_identifier = ident;
    let Some(adt) = adt_mut(main, log, root) else {
        return;
    };
    let adt = adt.get_or_insert_with(AnimData::default);
    // `tracks.new(prev=None)`: on top, the active one, then renamed.
    for t in &mut adt.nla_tracks {
        t.active = false;
    }
    let mut track = NlaTrack::new(track_name);
    track.name = SmolStr::new(track_name);
    track.strips.push(strip);
    adt.nla_tracks.push(track);
    let t = adt.nla_tracks.len() - 1;
    // `BKE_nlastrip_validate_name`
    let unique = unique_subname(
        &|n| {
            adt.nla_tracks.iter().enumerate().any(|(i, tr)| {
                tr.strips
                    .iter()
                    .enumerate()
                    .any(|(j, x)| (i, j) != (t, 0) && x.name == n)
            })
        },
        &action_name,
        NAME_MAX,
        "NlaStrip",
    );
    adt.nla_tracks[t].strips[0].name = unique;
    // `_strip.action_frame_start = action.frame_range[0]`, then the end (RNA setters: clamped,
    // bounds recalculated).
    for (prop, v) in [
        ("action_frame_start", range[0]),
        ("action_frame_end", range[1]),
    ] {
        if let Some(p) = frap_anim::rna::anim_data::srna_of(AdtLoc::Strip(t, 0)).find_prop(prop) {
            let _ = adt_set(adt, AdtLoc::Strip(t, 0), p, Value::Float(v));
        }
    }
    let track = &mut adt.nla_tracks[t];
    track.lock = true;
    track.mute = true;
}

/// `Action::get_frame_range_of_slot`.
fn slot_range(act: &Action, handle: i32) -> [f32; 2] {
    if act.use_frame_range {
        return act.frame_range;
    }
    let (mut min, mut max, mut found) = (f32::MAX, f32::MIN, false);
    for layer in &act.layers {
        for s in &layer.strips {
            let Some(k) = s.keyframe() else { continue };
            for bag in k.channelbags.iter().filter(|b| b.slot_handle == handle) {
                for fc in &bag.fcurves {
                    if let (Some(f), Some(l)) =
                        (fc.keyframe_points.first(), fc.keyframe_points.last())
                    {
                        min = min.min(f.co[0]);
                        max = max.max(l.co[0]);
                        found = true;
                    }
                }
            }
        }
    }
    if found { [min, max] } else { [0.0, 0.0] }
}

/// `restore_animation_on_object` for the pointer datablocks (cameras, lights, material node
/// trees and materials): the strip of the track named `track_name` becomes the active action
/// and slot; without such a track the action is unassigned.
pub(crate) fn restore(main: &mut Main, log: &mut ChangeLog, created: &Created, track_name: &str) {
    let mut roots: Vec<Root> = Vec::new();
    roots.extend(created.cameras.values().map(|r| Root::Id(r.any())));
    roots.extend(created.lights.values().map(|r| Root::Id(r.any())));
    for r in created.materials.values() {
        roots.push(Root::Embedded {
            owner: r.any(),
            slot: EmbeddedSlot::NodeTree,
        });
        roots.push(Root::Id(r.any()));
    }
    for root in roots {
        let Some(slot) = adt_mut(main, log, root) else {
            continue;
        };
        let Some(adt) = slot.as_mut() else { continue };
        let found = adt
            .nla_tracks
            .iter()
            .find(|t| t.name == track_name && !t.strips.is_empty())
            .map(|t| {
                (
                    t.strips[0].action,
                    t.strips[0].action_slot_handle,
                    t.strips[0].last_slot_identifier.clone(),
                )
            });
        match found {
            Some((action, handle, ident)) => {
                adt.action = action;
                adt.action_slot_handle = handle;
                if !ident.is_empty() {
                    adt.last_slot_identifier = ident;
                }
            }
            None => {
                if adt.action.is_some() {
                    adt.action_slot_handle = 0;
                }
                adt.action = None;
            }
        }
    }
}

/// The pointer part of `BlenderScene.create_animations` when no animation importer runs:
/// every animation from the last to the first, its datablocks stashed, then the first one
/// restored.
pub(crate) fn create(
    cx: &mut Gltf,
    main: &mut Main,
    log: &mut ChangeLog,
) -> Result<(), ImportError> {
    if cx.doc.animations.is_empty()
        || !cx
            .doc
            .extensions_used
            .iter()
            .any(|e| e == KhrAnimationPointer::NAME)
    {
        return Ok(());
    }
    let names = track_names(cx);
    let mut st = ImportState::of(cx, main);
    let start = main.scenes.get(cx.scene).map_or(1, |s| s.frame_start);
    for ai in (0..cx.doc.animations.len()).rev() {
        let mut cache = ActionCache::default();
        anim(cx, main, log, ai, &names[ai], &mut st, &mut cache)?;
        for (root, action, slot) in cache.needs_stash {
            simulate_stash(main, log, root, &names[ai], action, slot, start);
        }
    }
    restore(main, log, &st.created, &names[0]);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use frap_gltf::json::ChannelTarget;

    fn ch(pointer: &str) -> Channel {
        Channel {
            extensions: Default::default(),
            extras: None,
            sampler: 0,
            target: KhrAnimationPointer::target(pointer),
        }
    }

    #[test]
    fn dispatch_follows_the_add_on() {
        assert_eq!(
            dispatch(&ch("/cameras/1/perspective/yfov")),
            Some(Target::Camera(1))
        );
        assert_eq!(dispatch(&ch("/cameras/1/perspective/aspectRatio")), None);
        assert_eq!(
            dispatch(&ch(
                "/extensions/KHR_lights_punctual/lights/0/spot.innerConeAngle"
            )),
            Some(Target::Light(0))
        );
        assert_eq!(
            dispatch(&ch("/materials/2/pbrMetallicRoughness/baseColorFactor")),
            Some(Target::Material(2, MatPart::Pbr))
        );
        assert_eq!(
            dispatch(&ch("/materials/0/extensions/KHR_materials_ior/ior")),
            Some(Target::Material(0, MatPart::Ext(3)))
        );
        assert_eq!(
            dispatch(&ch(
                "/materials/0/extensions/KHR_materials_clearcoat/clearcoatNormalTexture/scale"
            )),
            Some(Target::Material(0, MatPart::Ext(5)))
        );
        assert_eq!(
            dispatch(&ch(
                "/materials/0/emissiveTexture/extensions/KHR_texture_transform/offset"
            )),
            Some(Target::Material(0, MatPart::TextureTransform(0)))
        );
        assert_eq!(
            dispatch(&ch(
                "/materials/0/pbrMetallicRoughness/baseColorTexture/extensions/KHR_texture_transform/scale"
            )),
            Some(Target::Material(0, MatPart::TextureTransform(3)))
        );
        assert_eq!(
            dispatch(&ch(
                "/materials/1/extensions/KHR_materials_clearcoat/clearcoatNormalTexture/extensions/KHR_texture_transform/offset"
            )),
            Some(Target::Material(1, MatPart::TextureTransform(13)))
        );
        // Not dispatched: a rotation, a texture the add-on's lists miss (its dispatch fails).
        assert_eq!(
            dispatch(&ch(
                "/materials/0/normalTexture/extensions/KHR_texture_transform/rotation"
            )),
            None
        );
        assert_eq!(
            dispatch(&ch(
                "/materials/0/extensions/KHR_materials_foo/fooTexture/extensions/KHR_texture_transform/offset"
            )),
            Some(Target::Material(0, MatPart::TextureTransform(17)))
        );
        assert_eq!(
            dispatch(&ch("/nodes/3/rotation")),
            Some(Target::Node {
                node: 3,
                path: "rotation".into()
            })
        );
        assert_eq!(
            dispatch(&ch("/meshes/0/weights")),
            Some(Target::MeshWeights { mesh: 0 })
        );
        let plain = Channel {
            target: ChannelTarget {
                extensions: Default::default(),
                extras: None,
                node: Some(0),
                path: AnimationPath::Translation,
            },
            ..ch("/x")
        };
        assert_eq!(dispatch(&plain), None);
        assert!(handles(&ch("/materials/0/alphaCutoff")));
        assert!(!handles(&ch("/nodes/0/scale")));
    }

    #[test]
    fn unused_names() {
        let taken = vec!["Anim".to_owned(), "Anim.001".to_owned()];
        assert_eq!(find_unused_name(&taken, "Anim"), "Anim.002");
        assert_eq!(find_unused_name(&taken, "Other"), "Other");
        let long = "x".repeat(70);
        assert_eq!(find_unused_name(&[], &long).len(), 63);
    }

    /// A light whose `energy` an action animates: exported in SCENE mode with pointers, the
    /// file validates, and importing it gives the light an action with a `LASlot` whose
    /// `energy` curve comes back through `· 683 / 683`, stashed on a muted, locked track named
    /// after the animation and assigned back (the first animation). It goes through
    /// `import_bytes`, so it also fails when the animation importer (WP 6.7, `IMPLEMENTED`)
    /// takes over without calling [`anim`], [`simulate_stash`] and [`restore`].
    #[test]
    fn light_energy_round_trips() {
        use frap_anim::{AnimData, Keyframe};
        let (mut main, win) = frap_scene::startup::factory();
        let mut log = ChangeLog::default();
        let la = main.lights.find("Light").unwrap();
        let mut act = Action::new("LightAction");
        let h = act.slot_add_for_id_type("LIGHT", "Light");
        let bag = act.channelbag_ensure(h);
        let i = bag.fcurve_create("energy", 0, None);
        let fc = &mut bag.fcurves[i];
        fc.keyframe_points.push(Keyframe::new(1.0, 100.0));
        fc.keyframe_points.push(Keyframe::new(5.0, 500.0));
        recalc_handles(fc);
        let a = main.actions.insert("LightAction", act, &mut log);
        main.lights.get_mut(la, &mut log).unwrap().animation_data = Some(AnimData {
            action: Some(a),
            action_slot_handle: h,
            ..AnimData::default()
        });
        let prefs = frap_scene::Preferences::default();
        let ctx = frap_scene::Context {
            main: &main,
            win: &win,
            prefs: &prefs,
        };
        let mut p = crate::export::settings::default_props();
        p.export_lights.value = true;
        p.export_pointer_animation.value = true;
        p.export_animation_mode.value =
            frap_spec_ops_io::export_scene::gltf::ExportAnimationMode::Scene;
        let s = crate::export::ExportSettings::from_props(&p);
        let out = crate::export::export(&main, &ctx, &s).unwrap();
        let doc = &out.document;
        assert_eq!(doc.animations.len(), 1);
        assert_eq!(doc.animations[0].name.as_deref(), Some("Light"));
        let r = frap_gltf::validate_file(&out.bytes).unwrap();
        assert!(r.is_ok(), "{r}");
        assert!(
            doc.extensions_used
                .iter()
                .any(|e| e == KhrAnimationPointer::NAME)
        );

        let (mut main2, win2) = frap_scene::startup::factory();
        let mut log2 = ChangeLog::default();
        let settings = super::super::ImportSettings::default();
        super::super::import_bytes(
            &mut main2,
            &win2,
            &out.bytes,
            std::path::Path::new("light.glb"),
            &settings,
            &mut log2,
        )
        .unwrap();
        let act = main2
            .actions
            .find("Light")
            .expect("an action per animation");
        let action = main2.actions.get(act).unwrap();
        assert_eq!(action.slots.len(), 1);
        assert_eq!(action.slots[0].identifier, "LASlot");
        assert_eq!(action.layers[0].name, "layer0");
        let fc = action.all_fcurves().next().unwrap();
        assert_eq!(fc.data_path, "energy");
        assert_eq!(fc.group.as_deref(), Some("Light"));
        let v: Vec<f32> = fc.keyframe_points.iter().map(|k| k.co[1]).collect();
        assert!(
            (v[0] - 100.0).abs() < 1e-3 && (v[4] - 500.0).abs() < 1e-3,
            "{v:?}"
        );
        assert!(
            fc.keyframe_points
                .iter()
                .all(|k| k.interpolation == Interpolation::Linear)
        );
        // The imported light is the one named after the glTF light ("Light.001" here).
        let la2 = main2
            .lights
            .iter()
            .find(|(_, l)| {
                l.animation_data
                    .as_ref()
                    .is_some_and(|a| a.action == Some(act))
            })
            .map(|(r, _)| r)
            .expect("the first animation is assigned back");
        let adt = main2
            .lights
            .get(la2)
            .unwrap()
            .animation_data
            .as_ref()
            .unwrap();
        assert_eq!(adt.nla_tracks.len(), 1);
        let t = &adt.nla_tracks[0];
        assert_eq!(t.name, "Light");
        assert!(t.mute && t.lock);
        assert_eq!(t.strips[0].name, "Light");
        assert_eq!(t.strips[0].frame_start, 1.0);
        // The scene's whole range is baked (factory: 1 to 250).
        assert_eq!(t.strips[0].frame_end, 250.0);
        assert_eq!(fc.keyframe_points.len(), 250);
        assert_eq!(t.strips[0].action_slot_handle, adt.action_slot_handle);
    }

    /// `(pointer, interpolation, input floats, output accessor type, output floats)`.
    type TestChannel<'a> = (&'a str, &'a str, &'a [f32], &'a str, &'a [f32]);

    /// A GLB with one perspective camera, one point light and the pointer channels.
    fn pointer_glb(channels: &[TestChannel]) -> Vec<u8> {
        let mut bin: Vec<u8> = Vec::new();
        let mut views = Vec::new();
        let mut accessors = Vec::new();
        let mut add = |data: &[f32], ty: &str, n: usize| {
            let off = bin.len();
            for f in data {
                bin.extend_from_slice(&f.to_le_bytes());
            }
            views.push(
                serde_json::json!({"buffer": 0, "byteOffset": off, "byteLength": 4 * data.len()}),
            );
            let mut a = serde_json::json!({
                "bufferView": views.len() - 1, "componentType": 5126,
                "count": data.len() / n, "type": ty,
            });
            if ty == "SCALAR" && !data.is_empty() {
                let min = data.iter().copied().fold(f32::INFINITY, f32::min);
                let max = data.iter().copied().fold(f32::NEG_INFINITY, f32::max);
                a["min"] = serde_json::json!([min]);
                a["max"] = serde_json::json!([max]);
            }
            accessors.push(a);
            accessors.len() - 1
        };
        let mut chans = Vec::new();
        let mut samplers = Vec::new();
        for (pointer, interp, input, ty, output) in channels {
            let n = match *ty {
                "VEC2" => 2,
                "VEC3" => 3,
                "VEC4" => 4,
                _ => 1,
            };
            let i = add(input, "SCALAR", 1);
            let o = add(output, ty, n);
            samplers.push(serde_json::json!({"input": i, "output": o, "interpolation": interp}));
            chans.push(serde_json::json!({
                "sampler": samplers.len() - 1,
                "target": {"path": "pointer", "extensions": {"KHR_animation_pointer": {"pointer": pointer}}},
            }));
        }
        let json = serde_json::json!({
            "asset": {"version": "2.0"},
            "scene": 0,
            "scenes": [{"nodes": [0, 1]}],
            "nodes": [
                {"name": "Cam", "camera": 0},
                {"name": "Lamp", "extensions": {"KHR_lights_punctual": {"light": 0}}},
            ],
            "cameras": [{"type": "perspective", "perspective": {"yfov": 0.8, "znear": 0.1, "zfar": 100.0}}],
            "extensions": {"KHR_lights_punctual": {"lights": [{"type": "point", "intensity": 10.0}]}},
            "extensionsUsed": ["KHR_lights_punctual", "KHR_animation_pointer"],
            "animations": [{"name": "A", "channels": chans, "samplers": samplers}],
            "accessors": accessors,
            "bufferViews": views,
            "buffers": [{"byteLength": bin.len()}],
        });
        let mut j = serde_json::to_vec(&json).unwrap();
        while !j.len().is_multiple_of(4) {
            j.push(b' ');
        }
        while !bin.len().is_multiple_of(4) {
            bin.push(0);
        }
        let total = 12 + 8 + j.len() + 8 + bin.len();
        let mut out = Vec::new();
        out.extend_from_slice(b"glTF");
        out.extend_from_slice(&2u32.to_le_bytes());
        out.extend_from_slice(&(total as u32).to_le_bytes());
        out.extend_from_slice(&(j.len() as u32).to_le_bytes());
        out.extend_from_slice(b"JSON");
        out.extend_from_slice(&j);
        out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
        out.extend_from_slice(b"BIN\0");
        out.extend_from_slice(&bin);
        out
    }

    /// Channels the add-on raises on are skipped with a WARNING (no zeroed keys, no padded
    /// components, no infinite / NaN values in F-curves); the valid one is imported.
    #[test]
    fn invalid_channels_are_skipped_with_a_warning() {
        let keys: &[f32] = &[0.0, 1.0];
        let glb = pointer_glb(&[
            // Valid.
            (
                "/cameras/0/perspective/zfar",
                "LINEAR",
                keys,
                "SCALAR",
                &[50.0, 80.0],
            ),
            // One output for two inputs.
            (
                "/cameras/0/perspective/znear",
                "LINEAR",
                keys,
                "SCALAR",
                &[0.5],
            ),
            // yfov 0 → infinite lens.
            (
                "/cameras/0/perspective/yfov",
                "LINEAR",
                keys,
                "SCALAR",
                &[0.8, 0.0],
            ),
            // A SCALAR colour.
            (
                "/extensions/KHR_lights_punctual/lights/0/color",
                "LINEAR",
                keys,
                "SCALAR",
                &[0.5, 1.0],
            ),
            // CUBICSPLINE needs three outputs per input.
            (
                "/extensions/KHR_lights_punctual/lights/0/intensity",
                "CUBICSPLINE",
                keys,
                "SCALAR",
                &[0.0, 1.0, 0.0, 0.0, 2.0],
            ),
        ]);
        let (mut main, win) = frap_scene::startup::factory();
        let mut log = ChangeLog::default();
        let settings = super::super::ImportSettings::default();
        let rep = super::super::import_bytes(
            &mut main,
            &win,
            &glb,
            std::path::Path::new("bad.glb"),
            &settings,
            &mut log,
        )
        .unwrap();
        let act = main.actions.find("A").expect("the valid channel's action");
        let paths: Vec<&str> = main
            .actions
            .get(act)
            .unwrap()
            .all_fcurves()
            .map(|f| f.data_path.as_str())
            .collect();
        assert_eq!(paths, ["clip_end"]);
        let skipped: Vec<&str> = rep
            .reports
            .iter()
            .filter(|r| r.level == ReportType::Warning)
            .map(|r| r.message.as_str())
            .filter(|m| m.contains("skipped:"))
            .collect();
        assert_eq!(skipped.len(), 4, "{skipped:?}");
        assert!(
            skipped[0].contains("/cameras/0/perspective/znear")
                && skipped[0].contains("1 output(s) for 2 input(s)")
        );
        assert!(skipped[1].contains("yfov") && skipped[1].contains("not finite"));
        assert!(skipped[2].contains("/color") && skipped[2].contains("needs 3"));
        assert!(skipped[3].contains("/intensity") && skipped[3].contains("5 output(s) for 2"));
    }

    /// A GLB with one triangle per material of `materials` (a 1x1 PNG as image 0) and the
    /// pointer channels of [`pointer_glb`].
    fn material_glb(materials: serde_json::Value, channels: &[TestChannel]) -> Vec<u8> {
        let glb = pointer_glb(channels);
        let jlen = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
        let mut json: serde_json::Value = serde_json::from_slice(&glb[20..20 + jlen]).unwrap();
        let mut bin = glb[20 + jlen + 8..].to_vec();
        let off = bin.len();
        for f in [0.0f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0] {
            bin.extend_from_slice(&f.to_le_bytes());
        }
        let n = materials.as_array().map_or(0, Vec::len);
        json["bufferViews"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({"buffer": 0, "byteOffset": off, "byteLength": 36}));
        let view = json["bufferViews"].as_array().unwrap().len() - 1;
        json["accessors"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({
                "bufferView": view, "componentType": 5126, "count": 3, "type": "VEC3",
                "min": [0.0, 0.0, 0.0], "max": [1.0, 1.0, 0.0],
            }));
        let pos = json["accessors"].as_array().unwrap().len() - 1;
        let prims: Vec<serde_json::Value> = (0..n)
            .map(|m| serde_json::json!({"attributes": {"POSITION": pos}, "material": m}))
            .collect();
        json["meshes"] = serde_json::json!([{"primitives": prims}]);
        json["nodes"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({"name": "Tri", "mesh": 0}));
        json["scenes"][0]["nodes"] = serde_json::json!([0, 1, 2]);
        json["materials"] = materials;
        json["images"] = serde_json::json!([{"uri": "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg=="}]);
        json["textures"] = serde_json::json!([{"source": 0}]);
        json["buffers"][0]["byteLength"] = serde_json::json!(bin.len());
        json["extensionsUsed"].as_array_mut().unwrap().extend([
            serde_json::json!("KHR_materials_unlit"),
            serde_json::json!("KHR_texture_transform"),
        ]);
        let mut j = serde_json::to_vec(&json).unwrap();
        while !j.len().is_multiple_of(4) {
            j.push(b' ');
        }
        let total = 12 + 8 + j.len() + 8 + bin.len();
        let mut out = Vec::new();
        out.extend_from_slice(b"glTF");
        out.extend_from_slice(&2u32.to_le_bytes());
        out.extend_from_slice(&(total as u32).to_le_bytes());
        out.extend_from_slice(&(j.len() as u32).to_le_bytes());
        out.extend_from_slice(b"JSON");
        out.extend_from_slice(&j);
        out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
        out.extend_from_slice(b"BIN\0");
        out.extend_from_slice(&bin);
        out
    }

    /// WP 7.7: `KHR_texture_transform` channels key the importer's Mapping node (offset through
    /// `texture_transform_gltf_to_blender` with the static rotation and scale, scale as is); a
    /// rotation channel is never dispatched (silently not imported); a channel into a texture
    /// without `KHR_texture_transform` is skipped with a WARNING (the add-on's dispatch fails).
    /// An opaque unlit material's colour keys the Emission's Color; its missing alpha socket
    /// (the add-on raises) is reported.
    #[test]
    fn texture_transform_and_unlit_channels() {
        let keys: &[f32] = &[0.0, 1.0];
        let materials = serde_json::json!([
            {"name": "Tt", "pbrMetallicRoughness": {"baseColorTexture": {"index": 0,
                "extensions": {"KHR_texture_transform": {"rotation": 0.5, "scale": [2.0, 4.0]}}}},
             "normalTexture": {"index": 0}},
            {"name": "Flat", "extensions": {"KHR_materials_unlit": {}},
             "pbrMetallicRoughness": {"baseColorFactor": [1.0, 0.0, 0.0, 1.0]}},
        ]);
        let tt =
            "/materials/0/pbrMetallicRoughness/baseColorTexture/extensions/KHR_texture_transform";
        let glb = material_glb(
            materials,
            &[
                (
                    &format!("{tt}/offset"),
                    "LINEAR",
                    keys,
                    "VEC2",
                    &[0.0, 0.0, 0.5, 0.25],
                ),
                (
                    &format!("{tt}/scale"),
                    "LINEAR",
                    keys,
                    "VEC2",
                    &[2.0, 4.0, 3.0, 1.0],
                ),
                (
                    &format!("{tt}/rotation"),
                    "LINEAR",
                    keys,
                    "SCALAR",
                    &[0.5, 1.0],
                ),
                (
                    "/materials/0/normalTexture/extensions/KHR_texture_transform/offset",
                    "LINEAR",
                    keys,
                    "VEC2",
                    &[0.0, 0.0, 1.0, 1.0],
                ),
                (
                    "/materials/1/pbrMetallicRoughness/baseColorFactor",
                    "LINEAR",
                    keys,
                    "VEC4",
                    &[1.0, 0.0, 0.0, 1.0, 0.0, 0.5, 1.0, 1.0],
                ),
            ],
        );
        let (mut main, win) = frap_scene::startup::factory();
        let mut log = ChangeLog::default();
        let settings = super::super::ImportSettings::default();
        let rep = super::super::import_bytes(
            &mut main,
            &win,
            &glb,
            std::path::Path::new("tt.glb"),
            &settings,
            &mut log,
        )
        .unwrap();
        let act = main.actions.get(main.actions.find("A").unwrap()).unwrap();
        let curves: Vec<(String, i32, Vec<[f32; 2]>)> = act
            .all_fcurves()
            .map(|f| {
                (
                    f.data_path.to_string(),
                    f.array_index,
                    f.keyframe_points.iter().map(|k| k.co).collect(),
                )
            })
            .collect();
        let paths: Vec<(&str, i32)> = curves.iter().map(|(p, i, _)| (p.as_str(), *i)).collect();
        assert_eq!(
            paths,
            [
                ("nodes[\"Mapping\"].inputs[1].default_value", 0),
                ("nodes[\"Mapping\"].inputs[1].default_value", 1),
                ("nodes[\"Mapping\"].inputs[3].default_value", 0),
                ("nodes[\"Mapping\"].inputs[3].default_value", 1),
                ("nodes[\"Emission\"].inputs[0].default_value", 0),
                ("nodes[\"Emission\"].inputs[0].default_value", 1),
                ("nodes[\"Emission\"].inputs[0].default_value", 2),
            ]
        );
        // offset [0.5, 0.25] with rotation 0.5 and the scale channel's [3, 1] at that key.
        let x = (0.5 + 1.0 * 0.5f64.sin()) as f32;
        let y = (1.0 - 0.25 - 1.0 * 0.5f64.cos()) as f32;
        let last = curves[0].2[1];
        assert!((last[1] - x).abs() < 1e-6, "{last:?}");
        assert!((curves[1].2[1][1] - y).abs() < 1e-6);
        // The first key: offset 0 with scale [2, 4].
        assert!((curves[0].2[0][1] - (4.0 * 0.5f64.sin()) as f32).abs() < 1e-6);
        let warnings: Vec<&str> = rep
            .reports
            .iter()
            .filter(|r| r.level == ReportType::Warning)
            .map(|r| r.message.as_str())
            .collect();
        assert!(
            warnings.iter().any(|w| w.contains("does not define")),
            "{warnings:?}"
        );
        assert!(
            warnings.iter().any(|w| w.contains("no alpha socket")),
            "{warnings:?}"
        );
        assert!(
            !warnings.iter().any(|w| w.contains("rotation")),
            "{warnings:?}"
        );
    }
}
