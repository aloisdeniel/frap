// SPDX-License-Identifier: GPL-3.0-or-later
//! Graph navigation shared by the pattern recognisers (`lower` side). Port of the parts of
//! `io_scene_gltf2/blender/exp/material/search_node_tree.py` (glTF-Blender-IO 5.2.41,
//! Apache-2.0) that M1 needs: the active Material Output and `get_node_socket` (the first
//! non-muted Principled BSDF linked to it), `NodeNav.move_back` through
//! reroutes, `get_constant` (unlinked sockets, RGB and Value nodes),
//! `detect_multiply_by_constant` (Mix MULTIPLY with Factor 1, Math MULTIPLY), the Image
//! Texture search behind a socket and the texture's UV source, Mapping transform and sampler.

use frap_nodes::{Node, SocketValue};
use frap_rna::{IdRef, ImageId, Report, ReportType, Value};

use crate::{ChannelSel, Filter, LowerCx, Sampler, TexRef, TexTransform, UvSource, Wrap};

/// Where an input socket gets its value from.
#[derive(Copy, Clone, Debug)]
pub enum Src<'t> {
    /// Unlinked: the socket's own default value.
    Const(&'t SocketValue),
    /// Linked from output `.1` (index in `node.outputs`) of node `.0` (reroutes skipped).
    Linked(&'t Node, usize),
    /// The socket does not exist.
    Missing,
}

/// A WARNING diagnostic for a node setup the recognisers do not understand.
pub fn unrecognised(out: &mut Vec<Report>, what: impl Into<String>) {
    out.push(Report::new(ReportType::Warning, what.into()));
}

impl<'a> LowerCx<'a> {
    /// The active Material Output (`is_active_output`): the first `ShaderNodeOutputMaterial`
    /// in `tree.nodes`, whatever its `target` (verified on 5.2.4: Blender flags the first
    /// output node of the tree, and the next one when it is removed; frap does not store the
    /// flag, so a script cannot move it yet).
    pub fn material_output(&self) -> Option<&'a Node> {
        self.tree
            .nodes
            .iter()
            .find(|n| n.bl_idname == "ShaderNodeOutputMaterial")
    }

    /// The node feeding the active output's `Surface` (reroutes skipped), for diagnostics.
    pub fn surface_shader(&self) -> Option<&'a Node> {
        let out = self.material_output()?;
        match self.src(out, "Surface") {
            Src::Linked(n, _) => Some(n),
            _ => None,
        }
    }

    /// The Principled BSDF every M1 pattern reads: `get_node_socket` — the first non-muted
    /// Principled BSDF in `tree.nodes` whose first output reaches the active Material Output
    /// (`check_if_is_linked_to_active_output`: directly, or through the first output of every
    /// node on the way, such as a Mix Shader or a reroute). Node groups arrive with M3.
    pub fn principled(&self) -> Option<&'a Node> {
        let active = self.material_output()?.uid;
        self.tree.nodes.iter().find(|n| {
            n.bl_idname == "ShaderNodeBsdfPrincipled" && !n.mute && self.reaches_output(n, active)
        })
    }

    /// `check_if_is_linked_to_active_output(node.outputs[0])`, iteratively, each node visited
    /// once (a cycle cannot reach the output any other way).
    fn reaches_output(&self, from: &'a Node, active: u32) -> bool {
        let mut seen = std::collections::BTreeSet::new();
        let mut stack = vec![from];
        while let Some(n) = stack.pop() {
            if !seen.insert(n.uid) {
                continue;
            }
            let Some(out) = n.outputs.first() else {
                continue;
            };
            for l in &self.tree.links {
                if l.from_node != n.uid || l.from_socket != out.identifier {
                    continue;
                }
                if l.to_node == active {
                    return true;
                }
                if let Some(to) = self.tree.node(l.to_node)
                    && to.bl_idname != "ShaderNodeOutputMaterial"
                {
                    stack.push(to);
                }
            }
        }
        false
    }

    /// The source of input `name` (first enabled socket with that name) of `node`.
    pub fn src(&self, node: &'a Node, name: &str) -> Src<'a> {
        match node.input_index(name) {
            Some(i) => self.src_at(node, i),
            None => Src::Missing,
        }
    }

    /// The source of `node.inputs[i]`.
    pub fn src_at(&self, node: &'a Node, i: usize) -> Src<'a> {
        let Some(s) = node.inputs.get(i) else {
            return Src::Missing;
        };
        let mut link = self.tree.link_into(node.uid, &s.identifier);
        let mut guard = 0;
        while let Some(l) = link {
            let Some(from) = self.tree.node(l.from_node) else {
                break;
            };
            if from.bl_idname == "NodeReroute" && guard <= self.tree.nodes.len() {
                guard += 1;
                match from.inputs.first() {
                    Some(inp) => match self.tree.link_into(from.uid, &inp.identifier) {
                        Some(next) => {
                            link = Some(next);
                            continue;
                        }
                        None => return Src::Const(&inp.default_value),
                    },
                    None => return Src::Missing,
                }
            }
            return match from.output_by_identifier(&l.from_socket) {
                Some(o) => Src::Linked(from, o),
                None => Src::Missing,
            };
        }
        Src::Const(&s.default_value)
    }

    /// `get_constant` for colour sockets: RGB of an unlinked socket or of an RGB node.
    pub fn const_rgb(&self, src: Src<'a>) -> Option<[f32; 3]> {
        match src {
            Src::Const(SocketValue::Color(c)) => Some([c[0], c[1], c[2]]),
            Src::Linked(n, o) if n.bl_idname == "ShaderNodeRGB" => match n.outputs[o].default_value
            {
                SocketValue::Color(c) => Some([c[0], c[1], c[2]]),
                _ => None,
            },
            _ => None,
        }
    }

    /// `get_constant` for value sockets: an unlinked socket or a Value node.
    pub fn const_f32(&self, src: Src<'a>) -> Option<f32> {
        match src {
            Src::Const(SocketValue::Float(f)) => Some(*f),
            Src::Linked(n, o) if n.bl_idname == "ShaderNodeValue" => {
                match n.outputs[o].default_value {
                    SocketValue::Float(f) => Some(f),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// `get_multiply_factors` for a colour socket: a Mix (RGBA, MULTIPLY, Factor 1) gives its
    /// `A` and `B` sources.
    pub fn color_multiply(&self, src: Src<'a>) -> Option<(Src<'a>, Src<'a>)> {
        let Src::Linked(n, _) = src else { return None };
        let is_mul = n.bl_idname == "ShaderNodeMix"
            && n.prop_str("data_type") == "RGBA"
            && n.prop_str("blend_type") == "MULTIPLY"
            && self.const_f32(self.src(n, "Factor")) == Some(1.0);
        is_mul.then(|| (self.src(n, "A"), self.src(n, "B")))
    }

    /// `get_multiply_factors` for a value socket: Math MULTIPLY gives its two inputs.
    pub fn value_multiply(&self, src: Src<'a>) -> Option<(Src<'a>, Src<'a>)> {
        let Src::Linked(n, _) = src else { return None };
        (n.bl_idname == "ShaderNodeMath" && n.prop_str("operation") == "MULTIPLY")
            .then(|| (self.src_at(n, 0), self.src_at(n, 1)))
    }

    /// `detect_multiply_by_constant` for colours: the constant and the other factor.
    pub fn color_times_const(&self, src: Src<'a>) -> Option<([f32; 3], Src<'a>)> {
        let (a, b) = self.color_multiply(src)?;
        if let Some(c) = self.const_rgb(a) {
            return Some((c, b));
        }
        self.const_rgb(b).map(|c| (c, a))
    }

    /// `detect_multiply_by_constant` for values: the constant and the other factor.
    pub fn value_times_const(&self, src: Src<'a>) -> Option<(f32, Src<'a>)> {
        let (a, b) = self.value_multiply(src)?;
        if let Some(c) = self.const_f32(a) {
            return Some((c, b));
        }
        self.const_f32(b).map(|c| (c, a))
    }

    /// The Image Texture node behind `src` and the channel used: its `Color` (RGB) or `Alpha`
    /// (A) output, or a Separate Color (RGB mode) output fed by its `Color` (R, G or B).
    pub fn image_behind(&self, src: Src<'a>) -> Option<(&'a Node, ChannelSel)> {
        let Src::Linked(n, o) = src else { return None };
        match n.bl_idname {
            "ShaderNodeTexImage" => match n.outputs[o].identifier.as_str() {
                "Color" => Some((n, ChannelSel::Rgb)),
                "Alpha" => Some((n, ChannelSel::A)),
                _ => None,
            },
            "ShaderNodeSeparateColor" if n.prop_str("mode") == "RGB" => {
                let channel = match n.outputs[o].identifier.as_str() {
                    "Red" => ChannelSel::R,
                    "Green" => ChannelSel::G,
                    "Blue" => ChannelSel::B,
                    _ => return None,
                };
                match self.image_behind(self.src(n, "Color"))? {
                    (tex, ChannelSel::Rgb) => Some((tex, channel)),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// The image of an Image Texture node (`None` when unset: the exporter skips it).
    pub fn image_of(&self, tex: &Node) -> Option<IdRef<ImageId>> {
        match tex.prop("image") {
            Some(Value::Id(Some(id))) => IdRef::from_any(*id),
            _ => None,
        }
    }

    /// The texture reference of Image Texture node `tex` read through `channel`: image,
    /// colour space, sampler (`interpolation`, `extension`) and UV source (UV Map node,
    /// Mapping node with POINT/VECTOR `vector_type`). Unsupported setups add a WARNING.
    pub fn tex_ref(
        &self,
        tex: &'a Node,
        channel: ChannelSel,
        diags: &mut Vec<Report>,
    ) -> Option<TexRef> {
        let image = self.image_of(tex)?;
        let colorspace_is_data = self.images.image(image).is_some_and(|im| {
            im.colorspace_settings.is_data || im.colorspace_settings.name == "Non-Color"
        });
        let filter = match tex.prop_str("interpolation") {
            "Closest" => Filter::Closest,
            "Cubic" | "Smart" => Filter::Cubic,
            _ => Filter::Linear,
        };
        let wrap = match tex.prop_str("extension") {
            "EXTEND" | "CLIP" => Wrap::Clamp,
            "MIRROR" => Wrap::Mirror,
            _ => Wrap::Repeat,
        };
        let (uv, transform) = self.uv_of(tex, diags);
        Some(TexRef {
            image,
            uv,
            channel,
            transform,
            sampler: Sampler {
                filter,
                wrap_s: wrap,
                wrap_t: wrap,
            },
            colorspace_is_data,
        })
    }

    fn uv_of(&self, tex: &'a Node, diags: &mut Vec<Report>) -> (UvSource, Option<TexTransform>) {
        let mut src = self.src(tex, "Vector");
        let mut transform = None;
        if let Src::Linked(n, _) = src
            && n.bl_idname == "ShaderNodeMapping"
        {
            transform = self.mapping_transform(n, diags);
            src = self.src(n, "Vector");
        }
        let uv = match src {
            Src::Const(_) | Src::Missing => UvSource::ActiveRender,
            Src::Linked(n, _) if n.bl_idname == "ShaderNodeUVMap" => match n.prop_str("uv_map") {
                "" => UvSource::ActiveRender,
                name => UvSource::Named(name.into()),
            },
            Src::Linked(n, o)
                if n.bl_idname == "ShaderNodeTexCoord" && n.outputs[o].identifier == "UV" =>
            {
                UvSource::ActiveRender
            }
            Src::Linked(n, _) => {
                unrecognised(
                    diags,
                    format!(
                        "Image Texture \"{}\": texture coordinates from \"{}\" are not supported; using the active UV map",
                        tex.name, n.name
                    ),
                );
                UvSource::ActiveRender
            }
        };
        (uv, transform)
    }

    /// `get_texture_transform_from_mapping_node` + `texture_transform_blender_to_gltf`.
    fn mapping_transform(&self, n: &'a Node, diags: &mut Vec<Report>) -> Option<TexTransform> {
        let vector_type = n.prop_str("vector_type");
        if !matches!(vector_type, "POINT" | "VECTOR") {
            unrecognised(
                diags,
                format!(
                    "Mapping \"{}\": vector_type {vector_type} is not supported; texture transform skipped",
                    n.name
                ),
            );
            return None;
        }
        let v3 = |name: &str| match n.input(name).map(|s| &s.default_value) {
            Some(SocketValue::Vector(v)) => [v[0], v[1], v[2]],
            _ => [0.0; 3],
        };
        let (loc, rot, scale) = (v3("Location"), v3("Rotation"), v3("Scale"));
        if rot[0] != 0.0 || rot[1] != 0.0 {
            unrecognised(
                diags,
                format!(
                    "Mapping \"{}\": only a Z rotation can be exported; texture transform skipped",
                    n.name
                ),
            );
            return None;
        }
        let offset = if vector_type == "VECTOR" {
            [0.0, 0.0]
        } else {
            [loc[0], loc[1]]
        };
        Some(blender_to_gltf_transform(
            offset,
            rot[2],
            [scale[0], scale[1]],
        ))
        .filter(|t| !is_identity(t))
    }
}

/// `texture_transform_blender_to_gltf` (computed in f64 like Python).
pub fn blender_to_gltf_transform(offset: [f32; 2], rotation: f32, scale: [f32; 2]) -> TexTransform {
    let (o0, o1, r, s1) = (
        f64::from(offset[0]),
        f64::from(offset[1]),
        f64::from(rotation),
        f64::from(scale[1]),
    );
    TexTransform {
        offset: [(o0 - s1 * r.sin()) as f32, (1.0 - o1 - s1 * r.cos()) as f32],
        rotation,
        scale,
    }
}

/// `texture_transform_gltf_to_blender` (computed in f64 like Python): Mapping node
/// (Location XY, Rotation Z, Scale XY) of a glTF texture transform.
pub fn gltf_to_blender_transform(t: &TexTransform) -> ([f32; 2], f32, [f32; 2]) {
    let (o0, o1, r, s1) = (
        f64::from(t.offset[0]),
        f64::from(t.offset[1]),
        f64::from(t.rotation),
        f64::from(t.scale[1]),
    );
    (
        [(o0 + s1 * r.sin()) as f32, (1.0 - o1 - s1 * r.cos()) as f32],
        t.rotation,
        t.scale,
    )
}

/// A glTF texture transform that changes nothing (the exporter omits it).
pub fn is_identity(t: &TexTransform) -> bool {
    t.offset == [0.0, 0.0] && t.rotation == 0.0 && t.scale == [1.0, 1.0]
}
