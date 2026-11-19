// SPDX-License-Identifier: GPL-3.0-or-later
//! Sampler of a texture: Image Texture `interpolation` and `extension`, plus the manual UV
//! wrapping chain the importer builds for mixed wrap modes (ARCHITECTURE §16.2).
//!
//! A per-texture pattern: every texture reference is read through [`sampler_of`] (from
//! `LowerCx::tex_ref`) and every raised Image Texture gets its wrapping through
//! [`raise_wrapping`] (from `RaiseCx::texture`), so the material-wide entry registered here
//! has nothing to do on its own.
//!
//! Lowering ports `gather_sampler` / `__gather_wrap` / `detect_manual_uv_wrapping`
//! (`io_scene_gltf2/blender/exp/sampler.py`, glTF-Blender-IO 5.2.41, Apache-2.0):
//! `Closest` → NEAREST, anything else LINEAR (`Cubic` and `Smart` are written LINEAR);
//! `EXTEND` and `CLIP` → CLAMP_TO_EDGE, `MIRROR` → MIRRORED_REPEAT, else REPEAT; a
//! `Separate XYZ → [Math WRAP 0..1 | Math PINGPONG 1] → Combine XYZ → Vector` chain overrides
//! each axis (REPEAT / MIRRORED_REPEAT; an axis passed through keeps the node's wrap).
//!
//! Raising ports the wrap part of `texture()` (`imp/texture.py`): equal wraps set
//! `extension`; mixed ones set `EXTEND` and build that chain.

use frap_nodes::{Node, SocketAddr, SocketValue};

use crate::graph::In;
use crate::{
    Filter, LowerCx, LoweredMaterial, MaterialPattern, RaiseCx, Sampler, TexRef, Wrap,
};

/// What `detect_manual_uv_wrapping` finds in front of an Image Texture's `Vector`.
#[derive(Copy, Clone, Debug)]
pub struct ManualWrapping<'a> {
    /// The wrap of the X axis (`None`: passed through).
    pub wrap_s: Option<Wrap>,
    /// The wrap of the Y axis (`None`: passed through).
    pub wrap_t: Option<Wrap>,
    /// The Separate XYZ's input: where the UV chain goes on (`next_socket`).
    pub next: In<'a>,
}

/// `detect_manual_uv_wrapping(blender_shader_node)`.
pub fn detect_manual_uv_wrapping<'a>(
    cx: &LowerCx<'a>,
    tex: &'a Node,
) -> Option<ManualWrapping<'a>> {
    let (comb, _) = cx.peek_back(In::named(tex, "Vector")?)?;
    if comb.bl_idname != "ShaderNodeCombineXYZ" {
        return None;
    }
    let mut sep: Option<&'a Node> = None;
    let mut wraps = [None, None];
    for (axis, soc) in ["X", "Y"].into_iter().enumerate() {
        let comb_in = In::named(comb, soc)?;
        let (node, _) = cx.peek_back(comb_in)?;
        let (wrap, prev) = match node.bl_idname {
            "ShaderNodeSeparateXYZ" => (None, cx.peek_back(comb_in)),
            "ShaderNodeMath" => {
                let k = |i: usize| {
                    cx.get_constant(In { node, index: i })
                        .and_then(crate::graph::Konst::value)
                };
                let wrap = match node.prop_str("operation") {
                    "PINGPONG" if k(1) == Some(1.0) => Wrap::Mirror,
                    "WRAP" if k(1) == Some(0.0) && k(2) == Some(1.0) => Wrap::Repeat,
                    _ => return None,
                };
                (Some(wrap), cx.peek_back(In { node, index: 0 }))
            }
            _ => return None,
        };
        let (prev_node, out) = prev?;
        if prev_node.bl_idname != "ShaderNodeSeparateXYZ" || prev_node.outputs[out].identifier != soc
        {
            return None;
        }
        match sep {
            None => sep = Some(prev_node),
            Some(s) if s.uid != prev_node.uid => return None,
            Some(_) => {}
        }
        wraps[axis] = wrap;
    }
    let sep = sep?;
    Some(ManualWrapping {
        wrap_s: wraps[0],
        wrap_t: wraps[1],
        next: In {
            node: sep,
            index: 0,
        },
    })
}

/// The sampler of Image Texture node `tex` (`gather_sampler`): filter from
/// `interpolation`, wraps from `extension`, overridden per axis by a manual wrapping chain.
pub fn sampler_of(tex: &Node, manual: Option<&ManualWrapping>) -> Sampler {
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
    let (mut wrap_s, mut wrap_t) = (wrap, wrap);
    if let Some(m) = manual {
        wrap_s = m.wrap_s.unwrap_or(wrap_s);
        wrap_t = m.wrap_t.unwrap_or(wrap_t);
    }
    Sampler {
        filter,
        wrap_s,
        wrap_t,
    }
}

/// The wrap part of the importer's `texture()`: sets `extension` of Image Texture `img`
/// for equal wraps; builds `Combine XYZ ← Math (WRAP | PINGPONG) ← Separate XYZ` for mixed
/// ones. `uv_socket` is the Image Texture's `Vector`; returns the socket the rest of the UV
/// chain links into, the new `x`, and whether a UV Map node is needed.
pub(crate) fn raise_wrapping(
    cx: &mut RaiseCx,
    tex: &TexRef,
    img: u32,
    uv_socket: SocketAddr,
    x: f32,
    y: f32,
) -> (SocketAddr, f32, bool) {
    let (ws, wt) = (tex.sampler.wrap_s, tex.sampler.wrap_t);
    if ws == wt {
        let ext = match ws {
            Wrap::Clamp => "EXTEND",
            Wrap::Mirror => "MIRROR",
            _ => "REPEAT",
        };
        cx.set_enum(img, "extension", ext);
        return (uv_socket, x, false);
    }
    let mut x = x;
    // Mixed wrap modes: per-axis WRAP / PINGPONG math on the UVs.
    cx.set_enum(img, "extension", "EXTEND");
    let com = cx.add_node("ShaderNodeCombineXYZ", [x - 140.0, y - 100.0]);
    let o = cx.output(com, "Vector");
    cx.link(uv_socket, o);
    let mut axis = [cx.input(com, "X"), cx.input(com, "Y")];
    x -= 200.0;
    for (i, wrap) in [ws, wt].into_iter().enumerate() {
        let op = match wrap {
            Wrap::Repeat => Some("WRAP"),
            Wrap::Mirror => Some("PINGPONG"),
            _ => None,
        };
        if let Some(op) = op {
            let m = cx.add_node("ShaderNodeMath", [x - 140.0, y + 30.0 - i as f32 * 200.0]);
            cx.set_enum(m, "operation", op);
            if op == "WRAP" {
                cx.set_input_at(m, 1, SocketValue::Float(0.0));
                cx.set_input_at(m, 2, SocketValue::Float(1.0));
            } else {
                cx.set_input_at(m, 1, SocketValue::Float(1.0));
            }
            cx.link(axis[i], SocketAddr::output(m, 0));
            axis[i] = SocketAddr::input(m, 0);
        }
    }
    x -= 200.0;
    let sep = cx.add_node("ShaderNodeSeparateXYZ", [x - 140.0, y - 100.0]);
    let (ox, oy) = (cx.output(sep, "X"), cx.output(sep, "Y"));
    cx.link(axis[0], ox);
    cx.link(axis[1], oy);
    let next = cx.input(sep, "Vector");
    x -= 200.0;
    (next, x, true)
}

/// Per-texture pattern: nothing material-wide to lower (see the module documentation).
fn lower(_: &LowerCx, _: &mut LoweredMaterial) {}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::MATERIAL_PATTERNS)]
static PATTERN: MaterialPattern = MaterialPattern {
    name: "sampler",
    // 2 is WP 3.6's volume pattern; the sampler has nothing to lower, so any free order fits.
    order: 5,
    lower,
    raise: None,
};

#[cfg(test)]
mod tests {
    use crate::graph::testkit::*;
    use crate::{Filter, Wrap};

    #[test]
    fn interpolation_extension_and_manual_wrapping() {
        let mut images = Images::default();
        let img = images.add("t", false);
        let mut m = mat("S");
        let t = image_node(&mut m, img);
        set_enum(&mut m, t, "interpolation", "Closest");
        set_enum(&mut m, t, "extension", "EXTEND");
        link(&mut m, (t, "Color"), (BSDF, "Base Color"));
        let s = lower_m(&m, &images).base_color.texture.unwrap().sampler;
        assert_eq!(
            (s.filter, s.wrap_s, s.wrap_t),
            (Filter::Closest, Wrap::Clamp, Wrap::Clamp)
        );
        // Separate XYZ → WRAP (X) / PINGPONG (Y) → Combine XYZ: REPEAT × MIRRORED_REPEAT.
        let sep = add(&mut m, "ShaderNodeSeparateXYZ");
        let comb = add(&mut m, "ShaderNodeCombineXYZ");
        let wrap = add(&mut m, "ShaderNodeMath");
        set_enum(&mut m, wrap, "operation", "WRAP");
        set_in_at(&mut m, wrap, 1, float(0.0));
        set_in_at(&mut m, wrap, 2, float(1.0));
        let pp = add(&mut m, "ShaderNodeMath");
        set_enum(&mut m, pp, "operation", "PINGPONG");
        set_in_at(&mut m, pp, 1, float(1.0));
        link(&mut m, (sep, "X"), (wrap, "Value"));
        link(&mut m, (sep, "Y"), (pp, "Value"));
        link_at(&mut m, (wrap, 0), (comb, 0));
        link_at(&mut m, (pp, 0), (comb, 1));
        link(&mut m, (comb, "Vector"), (t, "Vector"));
        // The importer feeds the Separate XYZ from a UV Map node (unlinked, the chain would
        // be constants that the inliner folds away).
        let uv = add(&mut m, "ShaderNodeUVMap");
        link(&mut m, (uv, "UV"), (sep, "Vector"));
        let lm = lower_m(&m, &images);
        let s = lm.base_color.texture.as_ref().unwrap().sampler;
        assert_eq!((s.wrap_s, s.wrap_t), (Wrap::Repeat, Wrap::Mirror));
        assert!(lm.diagnostics.is_empty(), "{:?}", lm.diagnostics);
        assert_law(&lm, &images);
        // A WRAP with other bounds is not a wrap mode: the chain is not recognised.
        set_in_at(&mut m, wrap, 2, float(2.0));
        let s = lower_m(&m, &images).base_color.texture.unwrap().sampler;
        assert_eq!((s.wrap_s, s.wrap_t), (Wrap::Clamp, Wrap::Clamp));
    }
}
