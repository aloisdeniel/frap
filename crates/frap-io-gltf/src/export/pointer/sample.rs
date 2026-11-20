// SPDX-License-Identifier: GPL-3.0-or-later
//! Sampling a registered asset at a frame: the add-on's `get_cache_data` for pointer data
//! (`material_nodetree_caching`, `light_nodetree_caching`, `camera_caching` of
//! `blender/exp/animation/sampled/sampling_cache.py`) and the per-key conversions of
//! `gather_data_sampled_keyframes` (`blender/exp/animation/sampled/data/keyframes.py`),
//! io_scene_gltf2 5.2.41 (Apache-2.0, The glTF-Blender-IO authors).
//!
//! Blender sets each frame (`scene.frame_set`) and reads the properties back; frap evaluates
//! the datablock's animation (`frap_anim::eval::evaluate_anim_for`, the NLA and the active
//! action) at the frame and reads every other property from the exported state. A property
//! is a node socket's `default_value` of the material's embedded tree, or a light / camera
//! field.
//!
//! Conversions, as the add-on writes them (several differ from the static export; they are
//! Blender's behaviour, kept on purpose):
//! - a base colour and its alpha merge into one RGBA value;
//! - `specularFactor` is doubled and clamped to 1, the excess multiplying the specular colour;
//! - `emissiveFactor` is the colour times the strength, normalised by its maximum above 1;
//!   `emissiveStrength` is that maximum (1 when nothing exceeds 1);
//! - `attenuationDistance` is `1 / Density` (`1e13` for 0);
//! - a light `intensity` is `energy · 683` for point and spot lights in the `SPEC` lighting
//!   mode, the energy itself otherwise (no `1 / 4π`, no area, no exposure, unlike the static
//!   intensity); `spot.outerConeAngle` is `spot_size / 2`; `spot.innerConeAngle` is the raw
//!   `spot_blend` (the add-on's `light_caching`, which would convert it, is never called);
//! - a camera's `yfov` is `yvof_blender_to_gltf(angle, …)` and the magnifications
//!   `ortho_scale · (side / max side) / 2` with the context scene's resolution.

use frap_anim::eval::props::{PathSeg, parse_path, root_struct};
use frap_anim::eval::{ActionLookup, evaluate_anim_for};
use frap_anim::{Action, AnimData};
use frap_nodes::{NodeTree, SocketValue};
use frap_rna::{ActionId, CameraId, IdRef, LightId, MaterialId};
use frap_scene::Main;
use frap_scene::generated::plain::RenderSettings;
use indexmap::IndexMap;

use super::registry::{Asset, DataKind, PathInfo};
use crate::export::cameras::yfov_blender_to_gltf;
use crate::export::lights::PBR_WATTS_TO_LUMENS;

/// `bpy.data.actions` for the evaluator.
pub struct Actions<'a>(pub &'a Main);

impl ActionLookup for Actions<'_> {
    fn action(&self, r: IdRef<ActionId>) -> Option<&Action> {
        self.0.actions.get(r)
    }
}

/// The animation data evaluated for an asset: the datablock's own and, for a material, its
/// node tree's (each possibly modified for an NLA track bake).
#[derive(Clone, Debug, Default)]
pub struct AssetAnim {
    /// `datablock.animation_data`
    pub id: Option<AnimData>,
    /// `material.node_tree.animation_data`
    pub tree: Option<AnimData>,
}

impl AssetAnim {
    /// The animation data as stored.
    pub fn of(main: &Main, kind: DataKind, asset: &Asset) -> AssetAnim {
        match kind {
            DataKind::Materials => {
                let m = asset
                    .id
                    .typed::<MaterialId>()
                    .and_then(|r| main.materials.get(r));
                AssetAnim {
                    id: m.and_then(|m| m.animation_data.clone()),
                    tree: m.and_then(|m| m.node_tree.animation_data.clone()),
                }
            }
            DataKind::Lights => AssetAnim {
                id: asset
                    .id
                    .typed::<LightId>()
                    .and_then(|r| main.lights.get(r))
                    .and_then(|l| l.animation_data.clone()),
                tree: None,
            },
            DataKind::Cameras => AssetAnim {
                id: asset
                    .id
                    .typed::<CameraId>()
                    .and_then(|r| main.cameras.get(r))
                    .and_then(|c| c.animation_data.clone()),
                tree: None,
            },
        }
    }
}

/// Evaluated channels of one animation data at a frame, parsed.
struct Channels(Vec<(Vec<PathSeg>, i32, f32)>);

impl Channels {
    fn eval(main: &Main, adt: Option<&AnimData>, id_type: &str, frame: i32) -> Channels {
        let Some(adt) = adt else {
            return Channels(Vec::new());
        };
        let out = evaluate_anim_for(adt, &Actions(main), frame as f32, root_struct(id_type));
        Channels(
            out.into_iter()
                .filter_map(|c| Some((parse_path(&c.data_path)?, c.array_index, c.value)))
                .collect(),
        )
    }

    /// The value written to `prop[index]` of the datablock itself.
    fn prop(&self, prop: &str, index: i32) -> Option<f32> {
        self.0.iter().find_map(|(p, i, v)| {
            (*i == index && matches!(p.as_slice(), [PathSeg::Prop(n)] if n == prop)).then_some(*v)
        })
    }

    /// The value written to component `index` of socket `sock` of the tree.
    fn socket(&self, tree: &NodeTree, sock: &SocketRef, index: i32) -> Option<f32> {
        self.0.iter().find_map(|(p, i, v)| {
            (*i == index && socket_of(tree, p).as_ref() == Some(sock)).then_some(*v)
        })
    }
}

/// A node socket named by a registry key or a channel path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SocketRef {
    /// Node name.
    pub node: String,
    /// An output socket (a Value / RGB node's value).
    pub output: bool,
    /// Index in `inputs` / `outputs`.
    pub index: usize,
}

/// The socket `nodes[…].inputs|outputs[…].default_value` names (a key resolves to the first
/// socket with that name or identifier, as RNA collection lookup does).
fn socket_of(tree: &NodeTree, segs: &[PathSeg]) -> Option<SocketRef> {
    let [
        PathSeg::Prop(nodes),
        PathSeg::Key(name),
        PathSeg::Prop(side),
        key,
        PathSeg::Prop(dv),
    ] = segs
    else {
        return None;
    };
    if nodes != "nodes" || dv != "default_value" {
        return None;
    }
    let output = match side.as_str() {
        "inputs" => false,
        "outputs" => true,
        _ => return None,
    };
    let node = tree.nodes.iter().find(|n| n.name == name.as_str())?;
    let sockets = if output { &node.outputs } else { &node.inputs };
    let index = match key {
        PathSeg::Index(i) => usize::try_from(*i).ok().filter(|i| *i < sockets.len())?,
        PathSeg::Key(k) => sockets
            .iter()
            .position(|s| s.name == k.as_str() || s.identifier == k.as_str())?,
        PathSeg::Prop(_) => return None,
    };
    Some(SocketRef {
        node: name.to_string(),
        output,
        index,
    })
}

/// The socket a registry key (`node_tree.nodes[…]…`) names.
pub fn key_socket(tree: &NodeTree, key: &str) -> Option<SocketRef> {
    let segs = parse_path(key)?;
    match segs.split_first()? {
        (PathSeg::Prop(nt), rest) if nt == "node_tree" => socket_of(tree, rest),
        _ => None,
    }
}

/// The components of a socket's default value, as the add-on reads them (`float` or the
/// whole array).
fn socket_value(v: &SocketValue) -> Vec<f32> {
    match v {
        SocketValue::Float(f) => vec![*f],
        SocketValue::Int(i) => vec![*i as f32],
        SocketValue::Bool(b) => vec![f32::from(u8::from(*b))],
        SocketValue::Color(c) | SocketValue::Vector(c) => c.to_vec(),
        SocketValue::Rotation(r) => r.to_vec(),
        _ => Vec::new(),
    }
}

/// One frame of an asset: the evaluated property values by registry key (the add-on's cache
/// data, before the keyframe conversions).
pub type FrameData = IndexMap<String, Vec<f64>>;

/// The material's property at `key` at the evaluated frame.
fn material_value(tree: &NodeTree, ch: &Channels, key: &str) -> Option<Vec<f64>> {
    let s = key_socket(tree, key)?;
    let node = tree.nodes.iter().find(|n| n.name == s.node.as_str())?;
    let sock = if s.output {
        node.outputs.get(s.index)?
    } else {
        node.inputs.get(s.index)?
    };
    let mut v = socket_value(&sock.default_value);
    for (i, c) in v.iter_mut().enumerate() {
        if let Some(x) = ch.socket(tree, &s, i as i32) {
            *c = x;
        }
    }
    Some(v.into_iter().map(f64::from).collect())
}

/// `list(val)[:length]` (a float stays one value).
fn take(v: Vec<f64>, length: usize) -> Vec<f64> {
    if v.len() <= 1 {
        v
    } else {
        v.into_iter().take(length).collect()
    }
}

/// The paths of `asset` sharing `pointer`'s parent whose last token is `last`.
fn sibling<'a>(asset: &'a Asset, pointer: &str, last: &str) -> Option<&'a str> {
    let parent = pointer.rsplit_once('/')?.0;
    asset.paths.iter().find_map(|(k, p)| {
        let (pp, l) = p.pointer.rsplit_once('/')?;
        (pp == parent && l == last).then_some(k.as_str())
    })
}

const BASE_COLOR: &str = "/materials/XXX/pbrMetallicRoughness/baseColorFactor";
const SPECULAR: &str = "/materials/XXX/extensions/KHR_materials_specular/specularFactor";
const SPECULAR_COLOR: &str = "/materials/XXX/extensions/KHR_materials_specular/specularColorFactor";

/// `material_nodetree_caching` for one frame.
fn material_frame(tree: &NodeTree, ch: &Channels, asset: &Asset) -> FrameData {
    let mut data = FrameData::new();
    let mut base_color_done = false;
    for (key, info) in &asset.paths {
        if !key.starts_with("node_tree") {
            continue;
        }
        let val = |k: &str| material_value(tree, ch, k);
        if info.pointer == BASE_COLOR {
            if base_color_done {
                continue;
            }
            base_color_done = true;
            let (color, alpha) = if info.length == 3 {
                let c = val(key).unwrap_or_default();
                let a = info
                    .additional_path
                    .as_deref()
                    .and_then(&val)
                    .and_then(|a| a.first().copied())
                    .unwrap_or(1.0);
                (c, a)
            } else {
                let a = val(key).and_then(|a| a.first().copied()).unwrap_or(1.0);
                let c = info
                    .additional_path
                    .as_deref()
                    .and_then(&val)
                    .unwrap_or_else(|| vec![1.0; 3]);
                (c, a)
            };
            let mut rgba: Vec<f64> = color.into_iter().take(3).collect();
            rgba.push(alpha);
            data.insert(key.clone(), rgba);
        } else if info.pointer == SPECULAR {
            let v = val(key).and_then(|v| v.first().copied()).unwrap_or(0.0) * 2.0;
            let (v, fac) = if v > 1.0 { (1.0, v) } else { (v, 1.0) };
            data.insert(key.clone(), vec![v]);
            if let Some(ck) = sibling(asset, &info.pointer, "specularColorFactor") {
                let mut c: Vec<f64> = val(ck).unwrap_or_default().into_iter().take(3).collect();
                if fac > 1.0 {
                    c.iter_mut().for_each(|x| *x *= fac);
                }
                data.insert(ck.to_owned(), c);
            }
        } else if info.pointer == SPECULAR_COLOR {
            // Written with the specular factor; read alone when there is none (the add-on
            // would fail on the missing data).
            if sibling(asset, &info.pointer, "specularFactor").is_none() {
                let c = val(key).unwrap_or_default();
                data.insert(key.clone(), take(c, info.length));
            }
        } else if let Some(v) = val(key) {
            data.insert(key.clone(), take(v, info.length));
        }
    }
    data
}

/// The camera fields the animation writes, with their stored values otherwise.
struct CameraState {
    lens: f32,
    sensor_width: f32,
    sensor_height: f32,
    sensor_fit: String,
    ortho_scale: f32,
    clip_start: f32,
    clip_end: f32,
}

/// `fov_to_focallength`
fn fov_to_focallength(hfov: f32, sensor: f32) -> f32 {
    (sensor / 2.0) / (hfov * 0.5).tan()
}

/// `light_nodetree_caching` / `camera_caching` and the material caching for one frame of
/// `asset` with the animation `anim`.
pub fn frame_data(
    main: &Main,
    render: &RenderSettings,
    kind: DataKind,
    asset: &Asset,
    anim: &AssetAnim,
    frame: i32,
) -> FrameData {
    match kind {
        DataKind::Materials => {
            let Some(mat) = asset
                .id
                .typed::<MaterialId>()
                .and_then(|r| main.materials.get(r))
            else {
                return FrameData::new();
            };
            let ch = Channels::eval(main, anim.tree.as_ref(), "NODETREE", frame);
            material_frame(&mat.node_tree, &ch, asset)
        }
        DataKind::Lights => {
            let Some(la) = asset.id.typed::<LightId>().and_then(|r| main.lights.get(r)) else {
                return FrameData::new();
            };
            let ch = Channels::eval(main, anim.id.as_ref(), "LIGHT", frame);
            let f = &la.fields;
            let get = |p: &str, i: i32, stored: f32| f64::from(ch.prop(p, i).unwrap_or(stored));
            let mut data = FrameData::new();
            for key in asset.paths.keys() {
                let v = match key.as_str() {
                    "color" => (0..3)
                        .map(|i| get("color", i, f.color[i as usize]))
                        .collect(),
                    "energy" => vec![get("energy", 0, f.energy)],
                    "spot_size" => vec![get("spot_size", 0, f.spot_size)],
                    "spot_blend" => vec![get("spot_blend", 0, f.spot_blend)],
                    "cutoff_distance" => vec![get("cutoff_distance", 0, f.cutoff_distance)],
                    _ => continue,
                };
                data.insert(key.clone(), v);
            }
            data
        }
        DataKind::Cameras => {
            let Some(cam) = asset
                .id
                .typed::<CameraId>()
                .and_then(|r| main.cameras.get(r))
            else {
                return FrameData::new();
            };
            let ch = Channels::eval(main, anim.id.as_ref(), "CAMERA", frame);
            let f = &cam.fields;
            let get = |p: &str, stored: f32| ch.prop(p, 0).unwrap_or(stored);
            let mut c = CameraState {
                lens: get("lens", f.lens),
                sensor_width: get("sensor_width", f.sensor_width),
                sensor_height: get("sensor_height", f.sensor_height),
                sensor_fit: f.sensor_fit.to_string(),
                ortho_scale: get("ortho_scale", f.ortho_scale),
                clip_start: get("clip_start", f.clip_start),
                clip_end: get("clip_end", f.clip_end),
            };
            // An animated angle writes the lens through the RNA setters.
            if let Some(a) = ch.prop("angle", 0) {
                let sensor = if c.sensor_fit == "VERTICAL" {
                    c.sensor_height
                } else {
                    c.sensor_width
                };
                c.lens = fov_to_focallength(a, sensor);
            }
            if let Some(a) = ch.prop("angle_x", 0) {
                c.lens = fov_to_focallength(a, c.sensor_width);
            }
            if let Some(a) = ch.prop("angle_y", 0) {
                c.lens = fov_to_focallength(a, c.sensor_height);
            }
            let width = f64::from(render.pixel_aspect_x) * f64::from(render.resolution_x);
            let height = f64::from(render.pixel_aspect_y) * f64::from(render.resolution_y);
            let mut data = FrameData::new();
            for (key, info) in &asset.paths {
                let v = match info.pointer.as_str() {
                    "/cameras/XXX/perspective/yfov" => {
                        // `Camera.angle` (float, `focallength_to_fov`).
                        let sensor = if c.sensor_fit == "VERTICAL" {
                            c.sensor_height
                        } else {
                            c.sensor_width
                        };
                        let angle = 2.0 * ((sensor / 2.0) / c.lens).atan();
                        yfov_blender_to_gltf(f64::from(angle), width, height, &c.sensor_fit)
                    }
                    "/cameras/XXX/orthographic/xmag" => {
                        f64::from(c.ortho_scale) * (width / width.max(height)) / 2.0
                    }
                    "/cameras/XXX/orthographic/ymag" => {
                        f64::from(c.ortho_scale) * (height / width.max(height)) / 2.0
                    }
                    _ => match key.as_str() {
                        "clip_start" => f64::from(c.clip_start),
                        "clip_end" => f64::from(c.clip_end),
                        _ => continue,
                    },
                };
                data.insert(key.clone(), vec![v]);
            }
            data
        }
    }
}

/// `gather_data_sampled_keyframes`' conversion of the cached value of `key` (`None` when there
/// is no data for it).
pub fn key_value(
    kind: DataKind,
    asset: &Asset,
    key: &str,
    data: &FrameData,
    spec_lighting: bool,
) -> Option<Vec<f64>> {
    let info: &PathInfo = asset.paths.get(key)?;
    let mut value = data.get(key)?.clone();
    match kind {
        DataKind::Materials => {
            if info.pointer.contains("attenuationDistance") {
                let v = value.first().copied().unwrap_or(0.0);
                value = vec![if v != 0.0 { 1.0 / v } else { 1e13 }];
            }
            if info.pointer == "/materials/XXX/emissiveFactor" {
                let strength = info
                    .strength_channel
                    .as_deref()
                    .and_then(|s| data.get(s))
                    .and_then(|s| s.first().copied())
                    .unwrap_or(1.0);
                value.iter_mut().for_each(|f| *f *= strength);
                if value.iter().any(|&f| f > 1.0) {
                    let m = value.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                    value.iter_mut().for_each(|f| *f /= m);
                }
            }
            if info.pointer
                == "/materials/XXX/extensions/KHR_materials_emissive_strength/emissiveStrength"
                && let Some(factor) = info.factor_channel.as_deref().and_then(|f| data.get(f))
            {
                let v = value.first().copied().unwrap_or(0.0);
                let factor: Vec<f64> = factor.iter().map(|f| f * v).collect();
                value = vec![if factor.iter().any(|&f| f > 1.0) {
                    factor.iter().copied().fold(f64::NEG_INFINITY, f64::max)
                } else {
                    1.0
                }];
            }
        }
        DataKind::Lights => {
            if info.pointer == "/extensions/KHR_lights_punctual/lights/XXX/intensity"
                && spec_lighting
                && info.lamp_type.as_deref() != Some("SUN")
            {
                value.iter_mut().for_each(|v| *v *= PBR_WATTS_TO_LUMENS);
            }
            if info.pointer == "/extensions/KHR_lights_punctual/lights/XXX/spot.outerConeAngle" {
                value.iter_mut().for_each(|v| *v *= 0.5);
            }
        }
        DataKind::Cameras => {}
    }
    Some(value)
}

/// `fcurve_is_constant`: every component's peak-to-peak below `1e-4`.
pub fn is_constant(keys: &[Vec<f64>]) -> bool {
    let n = keys.first().map_or(0, Vec::len);
    (0..n).all(|c| channel_is_constant(keys, c))
}

/// `fcurve_channel_is_constant(keyframes, channel)`.
pub fn channel_is_constant(keys: &[Vec<f64>], c: usize) -> bool {
    let (lo, hi) = keys
        .iter()
        .filter_map(|k| k.get(c))
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), &v| {
            (lo.min(v), hi.max(v))
        });
    hi - lo < 0.0001
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_checks_follow_numpy_ptp() {
        assert!(is_constant(&[vec![1.0], vec![1.00005]]));
        assert!(!is_constant(&[vec![1.0], vec![1.0002]]));
        let rgba = [vec![1.0, 0.0, 0.0, 1.0], vec![0.0, 0.0, 1.0, 1.0]];
        assert!(!is_constant(&rgba));
        assert!(channel_is_constant(&rgba, 3));
        assert!(!channel_is_constant(&rgba, 0));
    }

    #[test]
    fn socket_keys_resolve_by_index_name_and_identifier() {
        let (main, _) = frap_scene::startup::factory();
        let (_, mat) = main
            .materials
            .iter()
            .find(|(_, m)| m.header.name == "Material")
            .unwrap();
        let t = &mat.node_tree;
        let want = Some(SocketRef {
            node: "Principled BSDF".into(),
            output: false,
            index: 2,
        });
        assert_eq!(
            key_socket(
                t,
                "node_tree.nodes[\"Principled BSDF\"].inputs[2].default_value"
            ),
            want
        );
        let segs =
            parse_path("nodes['Principled BSDF'].inputs[\"Roughness\"].default_value").unwrap();
        assert_eq!(socket_of(t, &segs), want);
        assert_eq!(key_socket(t, "nodes[\"Principled BSDF\"].inputs[2]"), None);
        assert_eq!(
            key_socket(t, "node_tree.nodes[\"Nope\"].inputs[2].default_value"),
            None
        );
    }

    #[test]
    fn light_conversions() {
        let mut paths = super::super::registry::light_paths("POINT", false);
        paths.extend(super::super::registry::light_paths("SPOT", false));
        let (main, _) = frap_scene::startup::factory();
        let (light, _) = main.lights.iter().next().unwrap();
        let asset = Asset {
            id: light.any(),
            index: 0,
            paths,
        };
        let mut data = FrameData::new();
        data.insert("energy".into(), vec![100.0]);
        data.insert("spot_size".into(), vec![1.0]);
        data.insert("spot_blend".into(), vec![0.25]);
        let mut a = asset.clone();
        a.paths["energy"].lamp_type = Some("POINT".into());
        assert_eq!(
            key_value(DataKind::Lights, &a, "energy", &data, true),
            Some(vec![68300.0])
        );
        assert_eq!(
            key_value(DataKind::Lights, &a, "energy", &data, false),
            Some(vec![100.0])
        );
        a.paths["energy"].lamp_type = Some("SUN".into());
        assert_eq!(
            key_value(DataKind::Lights, &a, "energy", &data, true),
            Some(vec![100.0])
        );
        assert_eq!(
            key_value(DataKind::Lights, &a, "spot_size", &data, true),
            Some(vec![0.5])
        );
        assert_eq!(
            key_value(DataKind::Lights, &a, "spot_blend", &data, true),
            Some(vec![0.25])
        );
    }
}
