// SPDX-License-Identifier: GPL-3.0-or-later
//! The constant folding of `InlineShaderNodes` (the tree the glTF exporter reads): a socket
//! linked to a node whose multi-function gets only constants holds the computed value instead
//! of the link.
//!
//! Ported from blender/source/blender/nodes/intern/shader_nodes_inline.cc (Blender 5.2.4,
//! GPL-2.0-or-later): `handle_input_socket` (the last used link, else the socket's own value;
//! an unlinked input of a reroute or a muted node is *dangling*, and a dangling value makes the
//! consumer use its own value), `handle_output_socket__reroute`,
//! `handle_output_socket__internal_links` (muted nodes), `handle_implicit_conversion`
//! (blenkernel/intern/type_conversions.cc) and the multi-functions of the shader nodes that
//! have one: Value, Math (`math_op`), Clamp (node_shader_clamp.cc), Map Range
//! (node_shader_map_range.cc), Combine/Separate XYZ (node_shader_sepcomb_xyz.cc), Mix
//! (node_shader_mix.cc) and Mix (Legacy) (node_shader_mix_rgb.cc) with `ramp_blend`
//! (blenkernel/intern/material.cc), Vector Math (functions/intern/multi_function_common.cc,
//! BLI_math_vector.hh) and Gamma (node_shader_gamma.cc). The muted-node internal links follow
//! `find_internally_linked_input` / `get_internal_link_type_priority`
//! (blenkernel/intern/node_tree_update.cc).
//!
//! Not folded (left linked, as frap did before): the texture nodes, Color Ramp, the curve
//! nodes, Vector Rotate, Blackbody and Radial Tiling, which have multi-functions in Blender
//! too (docs/deviations.md §2). Node groups are inlined before (`lowered/inline.rs`, WP 8.12).

use std::collections::{BTreeSet, HashMap};

use frap_nodes::{Node, SocketType, SocketValue};
use frap_rna::Value;

use super::math_op;
use crate::LowerCx;

/// A constant value of a socket.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum FoldVal {
    /// VALUE
    Float(f32),
    /// VECTOR
    Vector([f32; 3]),
    /// RGBA
    Color([f32; 4]),
}

/// `IMB_colormanagement_get_luminance` with the default configuration's scene linear role
/// (Linear Rec.709: `luma: [0.2126, 0.7152, 0.0722]`).
const LUMA: [f32; 3] = [0.2126, 0.7152, 0.0722];

impl FoldVal {
    /// The value of an unlinked socket of type `ty`.
    pub fn of_socket(ty: SocketType, v: &SocketValue) -> Option<FoldVal> {
        match (ty, v) {
            (SocketType::Value, SocketValue::Float(f)) => Some(FoldVal::Float(*f)),
            (SocketType::Vector, SocketValue::Vector(v)) => {
                Some(FoldVal::Vector([v[0], v[1], v[2]]))
            }
            (SocketType::Rgba, SocketValue::Color(c)) => Some(FoldVal::Color(*c)),
            _ => None,
        }
    }

    /// The implicit conversion into a socket of type `ty` (`None`: not convertible, or a type
    /// frap does not fold).
    pub fn to(self, ty: SocketType) -> Option<FoldVal> {
        match ty {
            SocketType::Value => Some(FoldVal::Float(self.f())),
            SocketType::Vector => Some(FoldVal::Vector(self.v())),
            SocketType::Rgba => Some(FoldVal::Color(self.c())),
            _ => None,
        }
    }

    /// As a float: a vector's mean, a colour's luminance.
    pub fn f(self) -> f32 {
        match self {
            FoldVal::Float(f) => f,
            FoldVal::Vector(v) => (v[0] + v[1] + v[2]) / 3.0,
            FoldVal::Color(c) => LUMA[0] * c[0] + LUMA[1] * c[1] + LUMA[2] * c[2],
        }
    }

    /// As a vector.
    pub fn v(self) -> [f32; 3] {
        match self {
            FoldVal::Float(f) => [f; 3],
            FoldVal::Vector(v) => v,
            FoldVal::Color(c) => [c[0], c[1], c[2]],
        }
    }

    /// As a colour (alpha 1 from a float or a vector).
    pub fn c(self) -> [f32; 4] {
        match self {
            FoldVal::Float(f) => [f, f, f, 1.0],
            FoldVal::Vector(v) => [v[0], v[1], v[2], 1.0],
            FoldVal::Color(c) => c,
        }
    }
}

/// The value the inliner gives a socket.
#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) enum Res {
    /// A constant.
    Val(FoldVal),
    /// `DanglingValue`: the consumer uses its own value.
    Dangling,
    /// Not a constant (the inlined tree keeps a link).
    No,
}

/// One folding pass over a tree.
pub(crate) struct Folder<'c, 'a> {
    cx: &'c LowerCx<'a>,
    /// `is_valid` of each link.
    valid: Vec<bool>,
    memo: HashMap<(u32, usize), Res>,
    /// Nodes being evaluated (a cycle folds to nothing).
    on_path: BTreeSet<u32>,
}

impl<'c, 'a> Folder<'c, 'a> {
    pub(crate) fn new(cx: &'c LowerCx<'a>) -> Self {
        Folder {
            cx,
            valid: cx.tree.links_valid(),
            memo: HashMap::new(),
            on_path: BTreeSet::new(),
        }
    }

    /// The last used link into `node.inputs[k]`: (source node, output index).
    pub(crate) fn link_into(&self, node: &Node, k: usize) -> Option<(&'a Node, usize)> {
        let s = node.inputs.get(k)?;
        let tree = self.cx.tree;
        let l = tree
            .links
            .iter()
            .enumerate()
            .filter(|(li, l)| {
                l.to_node == node.uid && l.to_socket == s.identifier && self.valid[*li]
            })
            .map(|(_, l)| l)
            .next_back()?;
        let from = tree.node(l.from_node)?;
        Some((from, from.output_by_identifier(&l.from_socket)?))
    }

    /// `handle_input_socket` for `node.inputs[k]`.
    pub(crate) fn input(&mut self, node: &'a Node, k: usize) -> Res {
        let s = &node.inputs[k];
        let may_dangle = node.bl_idname == "NodeReroute" || node.mute;
        let own = || {
            if may_dangle {
                Res::Dangling
            } else {
                FoldVal::of_socket(s.ty, &s.default_value).map_or(Res::No, Res::Val)
            }
        };
        let Some((from, o)) = self.link_into(node, k) else {
            return own();
        };
        match self.output(from, o) {
            Res::Val(v) => v.to(s.ty).map_or(Res::No, Res::Val),
            Res::Dangling => own(),
            Res::No => Res::No,
        }
    }

    /// The value of output `o` of `n`.
    pub(crate) fn output(&mut self, n: &'a Node, o: usize) -> Res {
        if let Some(r) = self.memo.get(&(n.uid, o)) {
            return *r;
        }
        if !self.on_path.insert(n.uid) {
            return Res::No;
        }
        let r = self.eval(n, o);
        self.on_path.remove(&n.uid);
        self.memo.insert((n.uid, o), r);
        r
    }

    fn eval(&mut self, n: &'a Node, o: usize) -> Res {
        let Some(out) = n.outputs.get(o) else {
            return Res::No;
        };
        let out_ty = out.ty;
        let conv = |r: Res| match r {
            Res::Val(v) => v.to(out_ty).map_or(Res::No, Res::Val),
            r => r,
        };
        if n.bl_idname == "NodeReroute" {
            return if n.inputs.is_empty() {
                Res::No
            } else {
                conv(self.input(n, 0))
            };
        }
        if n.mute {
            return match internal_input(self.cx, n, o) {
                Some(k) => conv(self.input(n, k)),
                None => Res::Dangling,
            };
        }
        if !is_foldable(n.bl_idname) {
            return Res::No;
        }
        // Every available input must be a constant (`all_inputs_primitive`).
        let mut args: Vec<Option<FoldVal>> = vec![None; n.inputs.len()];
        for (k, arg) in args.iter_mut().enumerate() {
            if !n.inputs[k].enabled {
                continue;
            }
            match self.input(n, k) {
                Res::Val(v) => *arg = Some(v),
                _ => return Res::No,
            }
        }
        let a = Args { n, args: &args };
        eval_node(&a, o).map_or(Res::No, |v| conv(Res::Val(v)))
    }
}

/// The nodes with a multi-function that frap evaluates.
fn is_foldable(bl_idname: &str) -> bool {
    matches!(
        bl_idname,
        "ShaderNodeValue"
            | "ShaderNodeMath"
            | "ShaderNodeClamp"
            | "ShaderNodeMapRange"
            | "ShaderNodeCombineXYZ"
            | "ShaderNodeSeparateXYZ"
            | "ShaderNodeMix"
            | "ShaderNodeMixRGB"
            | "ShaderNodeVectorMath"
            | "ShaderNodeGamma"
    )
}

/// `get_internal_link_type_priority(from, to)`.
fn link_priority(from: SocketType, to: SocketType) -> i32 {
    use SocketType as T;
    match to {
        T::Rgba | T::Vector => match from {
            f if f == to => 4,
            T::Value => 3,
            T::Int => 2,
            T::Boolean => 1,
            _ => -1,
        },
        T::Value | T::Int | T::Boolean => {
            let order: [SocketType; 5] = match to {
                T::Value => [T::Value, T::Int, T::Boolean, T::Rgba, T::Vector],
                T::Int => [T::Int, T::Value, T::Boolean, T::Rgba, T::Vector],
                _ => [T::Boolean, T::Int, T::Value, T::Rgba, T::Vector],
            };
            order
                .iter()
                .position(|t| *t == from)
                .map_or(-1, |p| 5 - p as i32)
        }
        T::Rotation => match from {
            T::Rotation => 3,
            T::Vector => 2,
            T::Value => 1,
            _ => -1,
        },
        _ if from == to => 1,
        _ => -1,
    }
}

/// Inputs declared `no_muted_links()`.
fn no_muted_links(n: &Node, identifier: &str) -> bool {
    match n.bl_idname {
        "ShaderNodeMix" => matches!(identifier, "Factor_Float" | "Factor_Vector"),
        "ShaderNodeRGBCurve" | "ShaderNodeVectorCurve" | "ShaderNodeFloatCurve" => {
            identifier == "Fac" || identifier == "Factor"
        }
        _ => false,
    }
}

/// `find_internally_linked_input`: the input a muted node passes to output `o`.
pub(crate) fn internal_input(cx: &LowerCx, n: &Node, o: usize) -> Option<usize> {
    let out = n.outputs.get(o)?;
    if !out.enabled {
        return None;
    }
    let mut sel = None;
    let (mut sel_prio, mut sel_linked) = (-1, false);
    for (k, s) in n.inputs.iter().enumerate() {
        if !s.enabled || no_muted_links(n, &s.identifier) {
            continue;
        }
        let prio = link_priority(s.ty, out.ty);
        if prio < 0 {
            continue;
        }
        let linked = cx.tree.link_into(n.uid, &s.identifier).is_some();
        if !(prio > sel_prio || (linked && !sel_linked)) {
            continue;
        }
        sel = Some(k);
        sel_prio = prio;
        sel_linked = linked;
    }
    sel
}

/// The constant inputs of a node being folded.
struct Args<'n> {
    n: &'n Node,
    args: &'n [Option<FoldVal>],
}

impl Args<'_> {
    fn by_id(&self, identifier: &str) -> Option<FoldVal> {
        let k = self.n.input_by_identifier(identifier)?;
        self.args[k]
    }
    fn f(&self, identifier: &str) -> Option<f32> {
        self.by_id(identifier).map(FoldVal::f)
    }
    fn v(&self, identifier: &str) -> Option<[f32; 3]> {
        self.by_id(identifier).map(FoldVal::v)
    }
    fn c(&self, identifier: &str) -> Option<[f32; 4]> {
        self.by_id(identifier).map(FoldVal::c)
    }
    fn at(&self, k: usize) -> Option<FoldVal> {
        self.args.get(k).copied().flatten()
    }
    fn flag(&self, prop: &str) -> bool {
        matches!(self.n.prop(prop), Some(Value::Bool(true)))
    }
    fn out_id(&self, o: usize) -> &str {
        self.n.outputs[o].identifier.as_str()
    }
}

fn eval_node(a: &Args, o: usize) -> Option<FoldVal> {
    let n = a.n;
    Some(match n.bl_idname {
        "ShaderNodeValue" => match n.outputs.get(o)?.default_value {
            SocketValue::Float(f) => FoldVal::Float(f),
            _ => return None,
        },
        "ShaderNodeMath" => {
            let mut args = [0.0f32; 3];
            for (k, x) in args.iter_mut().enumerate() {
                if let Some(v) = a.at(k) {
                    *x = v.f();
                }
            }
            let r = math_op(n.prop_str("operation"), args)?;
            FoldVal::Float(if a.flag("use_clamp") {
                r.clamp(0.0, 1.0)
            } else {
                r
            })
        }
        "ShaderNodeClamp" => {
            let (v, lo, hi) = (a.f("Value")?, a.f("Min")?, a.f("Max")?);
            FoldVal::Float(match n.prop_str("clamp_type") {
                "RANGE" if lo < hi => clamp_f(v, lo, hi),
                "RANGE" => clamp_f(v, hi, lo),
                _ => v.max(lo).min(hi),
            })
        }
        "ShaderNodeMapRange" => map_range(a, o)?,
        "ShaderNodeCombineXYZ" => FoldVal::Vector([a.f("X")?, a.f("Y")?, a.f("Z")?]),
        "ShaderNodeSeparateXYZ" => {
            let v = a.v("Vector")?;
            FoldVal::Float(match a.out_id(o) {
                "X" => v[0],
                "Y" => v[1],
                "Z" => v[2],
                _ => return None,
            })
        }
        "ShaderNodeMix" => mix(a, o)?,
        "ShaderNodeMixRGB" => {
            let mut r = a.c("Color1")?;
            ramp_blend(
                n.prop_str("blend_type"),
                &mut r,
                clamp_f(a.f("Fac")?, 0.0, 1.0),
                &a.c("Color2")?,
            )?;
            if a.flag("use_clamp") {
                r = r.map(|x| x.clamp(0.0, 1.0));
            }
            FoldVal::Color(r)
        }
        "ShaderNodeVectorMath" => vector_math(a, o)?,
        "ShaderNodeGamma" => {
            let (c, g) = (a.c("Color")?, a.f("Gamma")?);
            FoldVal::Color([
                safe_pow(c[0], g),
                safe_pow(c[1], g),
                safe_pow(c[2], g),
                c[3],
            ])
        }
        _ => return None,
    })
}

fn clamp_f(v: f32, lo: f32, hi: f32) -> f32 {
    if v < lo {
        lo
    } else if v > hi {
        hi
    } else {
        v
    }
}

fn safe_divide(a: f32, b: f32) -> f32 {
    if b == 0.0 { 0.0 } else { a / b }
}

/// `safe_powf`
fn safe_pow(a: f32, b: f32) -> f32 {
    if a < 0.0 && b != (b as i32) as f32 {
        0.0
    } else {
        a.powf(b)
    }
}

/// `clamp_range` of Map Range: between the two bounds, in either order.
fn clamp_range(v: f32, a: f32, b: f32) -> f32 {
    if a > b { v.clamp(b, a) } else { v.clamp(a, b) }
}

/// Map Range (FLOAT and FLOAT_VECTOR; LINEAR, STEPPED, SMOOTHSTEP, SMOOTHERSTEP).
fn map_range(a: &Args, o: usize) -> Option<FoldVal> {
    let vector = a.n.prop_str("data_type") == "FLOAT_VECTOR";
    let interp = a.n.prop_str("interpolation_type");
    let clamp = a.flag("clamp") && matches!(interp, "LINEAR" | "STEPPED");
    let one = |v: f32, fmin: f32, fmax: f32, tmin: f32, tmax: f32, steps: f32| -> Option<f32> {
        let mut factor = safe_divide(v - fmin, fmax - fmin);
        match interp {
            "LINEAR" => {}
            "STEPPED" => factor = safe_divide((factor * (steps + 1.0)).floor(), steps),
            "SMOOTHSTEP" => {
                factor = factor.clamp(0.0, 1.0);
                factor = (3.0 - 2.0 * factor) * (factor * factor);
            }
            "SMOOTHERSTEP" => {
                factor = factor.clamp(0.0, 1.0);
                factor = factor * factor * factor * (factor * (factor * 6.0 - 15.0) + 10.0);
            }
            _ => return None,
        }
        let r = tmin + factor * (tmax - tmin);
        Some(if clamp { clamp_range(r, tmin, tmax) } else { r })
    };
    if vector {
        if a.out_id(o) != "Vector" {
            return None;
        }
        let (v, fmin, fmax) = (
            a.v("Vector")?,
            a.v("From_Min_FLOAT3")?,
            a.v("From_Max_FLOAT3")?,
        );
        let (tmin, tmax) = (a.v("To_Min_FLOAT3")?, a.v("To_Max_FLOAT3")?);
        let steps = if interp == "STEPPED" {
            a.v("Steps_FLOAT3")?
        } else {
            [0.0; 3]
        };
        let mut r = [0.0; 3];
        for i in 0..3 {
            r[i] = one(v[i], fmin[i], fmax[i], tmin[i], tmax[i], steps[i])?;
        }
        Some(FoldVal::Vector(r))
    } else {
        if a.out_id(o) != "Result" {
            return None;
        }
        let steps = if interp == "STEPPED" {
            a.f("Steps")?
        } else {
            0.0
        };
        one(
            a.f("Value")?,
            a.f("From Min")?,
            a.f("From Max")?,
            a.f("To Min")?,
            a.f("To Max")?,
            steps,
        )
        .map(FoldVal::Float)
    }
}

/// Mix (FLOAT, VECTOR uniform or not, RGBA with its blend modes).
fn mix(a: &Args, o: usize) -> Option<FoldVal> {
    let clamp_factor = a.flag("clamp_factor");
    let cf = |t: f32| if clamp_factor { t.clamp(0.0, 1.0) } else { t };
    match a.n.prop_str("data_type") {
        "FLOAT" => {
            if a.out_id(o) != "Result_Float" {
                return None;
            }
            let t = cf(a.f("Factor_Float")?);
            let (x, y) = (a.f("A_Float")?, a.f("B_Float")?);
            Some(FoldVal::Float(x * (1.0 - t) + y * t))
        }
        "VECTOR" => {
            if a.out_id(o) != "Result_Vector" {
                return None;
            }
            let t = if a.n.prop_str("factor_mode") == "NON_UNIFORM" {
                a.v("Factor_Vector")?.map(cf)
            } else {
                [cf(a.f("Factor_Float")?); 3]
            };
            let (x, y) = (a.v("A_Vector")?, a.v("B_Vector")?);
            Some(FoldVal::Vector(
                [0, 1, 2].map(|i| x[i] * (1.0 - t[i]) + y[i] * t[i]),
            ))
        }
        "RGBA" => {
            if a.out_id(o) != "Result_Color" {
                return None;
            }
            let mut r = a.c("A_Color")?;
            ramp_blend(
                a.n.prop_str("blend_type"),
                &mut r,
                cf(a.f("Factor_Float")?),
                &a.c("B_Color")?,
            )?;
            if a.flag("clamp_result") {
                r = r.map(|x| x.clamp(0.0, 1.0));
            }
            Some(FoldVal::Color(r))
        }
        _ => None,
    }
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// `math::normalize`: zero below a squared length of 1e-35.
fn normalize(v: [f32; 3]) -> [f32; 3] {
    let l2 = dot(v, v);
    if l2 > 1.0e-35 {
        let l = l2.sqrt();
        v.map(|x| x / l)
    } else {
        [0.0; 3]
    }
}

fn zip3(a: [f32; 3], b: [f32; 3], f: impl Fn(f32, f32) -> f32) -> [f32; 3] {
    [f(a[0], b[0]), f(a[1], b[1]), f(a[2], b[2])]
}

/// Vector Math (every operation; `Vector` and `Value` outputs).
fn vector_math(a: &Args, o: usize) -> Option<FoldVal> {
    let op = a.n.prop_str("operation");
    let x = || a.v("Vector");
    let y = || a.v("Vector_001");
    let z = || a.v("Vector_002");
    let float_out = matches!(op, "DOT_PRODUCT" | "DISTANCE" | "LENGTH");
    if a.out_id(o) != if float_out { "Value" } else { "Vector" } {
        return None;
    }
    let math = |name: &str, p: f32, q: f32, r: f32| math_op(name, [p, q, r]);
    let v = match op {
        "ADD" => zip3(x()?, y()?, |p, q| p + q),
        "SUBTRACT" => zip3(x()?, y()?, |p, q| p - q),
        "MULTIPLY" => zip3(x()?, y()?, |p, q| p * q),
        "DIVIDE" => zip3(x()?, y()?, safe_divide),
        "MULTIPLY_ADD" => {
            let (p, q, r) = (x()?, y()?, z()?);
            [0, 1, 2].map(|i| p[i] * q[i] + r[i])
        }
        "CROSS_PRODUCT" => {
            // `cross_high_precision`
            let (p, q) = (x()?.map(f64::from), y()?.map(f64::from));
            [
                (p[1] * q[2] - p[2] * q[1]) as f32,
                (p[2] * q[0] - p[0] * q[2]) as f32,
                (p[0] * q[1] - p[1] * q[0]) as f32,
            ]
        }
        "PROJECT" => {
            let (p, q) = (x()?, y()?);
            if q == [0.0; 3] {
                [0.0; 3]
            } else {
                let s = dot(p, q) / dot(q, q);
                q.map(|c| c * s)
            }
        }
        "REFLECT" => {
            let (i, n) = (x()?, normalize(y()?));
            let d = 2.0 * dot(n, i);
            [0, 1, 2].map(|k| i[k] - d * n[k])
        }
        "REFRACT" => {
            let (i, n, eta) = (x()?, normalize(y()?), a.f("Scale")?);
            let d = dot(n, i);
            let k = 1.0 - eta * eta * (1.0 - d * d);
            if k < 0.0 {
                [0.0; 3]
            } else {
                let s = eta * d + k.sqrt();
                [0, 1, 2].map(|c| eta * i[c] - s * n[c])
            }
        }
        "FACEFORWARD" => {
            let (p, i, r) = (x()?, y()?, z()?);
            if dot(r, i) < 0.0 { p } else { p.map(|c| -c) }
        }
        "DOT_PRODUCT" => return Some(FoldVal::Float(dot(x()?, y()?))),
        "DISTANCE" => {
            let d = zip3(x()?, y()?, |p, q| p - q);
            return Some(FoldVal::Float(dot(d, d).sqrt()));
        }
        "LENGTH" => {
            let p = x()?;
            return Some(FoldVal::Float(dot(p, p).sqrt()));
        }
        "SCALE" => {
            let s = a.f("Scale")?;
            x()?.map(|c| c * s)
        }
        "NORMALIZE" => normalize(x()?),
        "SNAP" => zip3(x()?, y()?, |p, q| safe_divide(p, q).floor() * q),
        "ROUND" => x()?.map(|c| (c + 0.5).floor()),
        "FLOOR" => x()?.map(f32::floor),
        "CEIL" => x()?.map(f32::ceil),
        "MODULO" => zip3(x()?, y()?, |p, q| if q == 0.0 { 0.0 } else { p % q }),
        "FRACTION" => x()?.map(|c| c - c.floor()),
        "ABSOLUTE" => x()?.map(f32::abs),
        "MINIMUM" => zip3(x()?, y()?, f32::min),
        "MAXIMUM" => zip3(x()?, y()?, f32::max),
        "WRAP" => {
            let (p, q, r) = (x()?, y()?, z()?);
            let mut out = [0.0; 3];
            for i in 0..3 {
                out[i] = math("WRAP", p[i], q[i], r[i])?;
            }
            out
        }
        "SINE" => x()?.map(f32::sin),
        "COSINE" => x()?.map(f32::cos),
        "TANGENT" => x()?.map(f32::tan),
        "POWER" => zip3(x()?, y()?, safe_pow),
        "SIGN" => x()?.map(|c| math("SIGN", c, 0.0, 0.0).unwrap_or(0.0)),
        _ => return None,
    };
    Some(FoldVal::Vector(v))
}

/// `rgb_to_hsv` (BLI_math_color).
fn rgb_to_hsv(mut r: f32, mut g: f32, mut b: f32) -> [f32; 3] {
    let mut k = 0.0f32;
    if g < b {
        std::mem::swap(&mut g, &mut b);
        k = -1.0;
    }
    let mut min_gb = b;
    if r < g {
        std::mem::swap(&mut r, &mut g);
        k = -2.0 / 6.0 - k;
        min_gb = g.min(b);
    }
    let chroma = r - min_gb;
    [
        (k + (g - b) / (6.0 * chroma + 1e-20)).abs(),
        chroma / (r + 1e-20),
        r,
    ]
}

/// `hsv_to_rgb` (BLI_math_color).
fn hsv_to_rgb(h: f32, s: f32, v: f32) -> [f32; 3] {
    let nr = ((h * 6.0 - 3.0).abs() - 1.0).clamp(0.0, 1.0);
    let ng = (2.0 - (h * 6.0 - 2.0).abs()).clamp(0.0, 1.0);
    let nb = (2.0 - (h * 6.0 - 4.0).abs()).clamp(0.0, 1.0);
    [nr, ng, nb].map(|n| ((n - 1.0) * s + 1.0) * v)
}

/// `ramp_blend(type, r_col, fac, col)` (blenkernel/intern/material.cc), by the RNA name of
/// the blend type. `None` for an unknown one.
fn ramp_blend(ty: &str, r: &mut [f32; 4], fac: f32, col: &[f32; 4]) -> Option<()> {
    let facm = 1.0 - fac;
    let each = |r: &mut [f32; 4], f: &dyn Fn(f32, f32) -> f32| {
        for i in 0..3 {
            r[i] = f(r[i], col[i]);
        }
    };
    match ty {
        "MIX" => {
            for i in 0..4 {
                r[i] = facm * r[i] + fac * col[i];
            }
        }
        "ADD" => each(r, &|x, c| x + fac * c),
        "MULTIPLY" => each(r, &|x, c| x * (facm + fac * c)),
        "SCREEN" => each(r, &|x, c| 1.0 - (facm + fac * (1.0 - c)) * (1.0 - x)),
        "OVERLAY" => each(r, &|x, c| {
            if x < 0.5 {
                x * (facm + 2.0 * fac * c)
            } else {
                1.0 - (facm + 2.0 * fac * (1.0 - c)) * (1.0 - x)
            }
        }),
        "SUBTRACT" => each(r, &|x, c| x - fac * c),
        "DIVIDE" => each(r, &|x, c| if c != 0.0 { facm * x + fac * x / c } else { x }),
        "DIFFERENCE" => each(r, &|x, c| facm * x + fac * (x - c).abs()),
        "EXCLUSION" => each(r, &|x, c| (facm * x + fac * (x + c - 2.0 * x * c)).max(0.0)),
        "DARKEN" => each(r, &|x, c| x.min(c) * fac + x * facm),
        "LIGHTEN" => each(r, &|x, c| x.max(c) * fac + x * facm),
        "DODGE" => each(r, &|x, c| {
            if x == 0.0 {
                return x;
            }
            let tmp = 1.0 - fac * c;
            if tmp <= 0.0 {
                1.0
            } else {
                let t = x / tmp;
                if t > 1.0 { 1.0 } else { t }
            }
        }),
        "BURN" => each(r, &|x, c| {
            let tmp = facm + fac * c;
            if tmp <= 0.0 {
                return 0.0;
            }
            // NaN stays NaN, as in the C comparisons.
            (1.0 - (1.0 - x) / tmp).clamp(0.0, 1.0)
        }),
        "HUE" => {
            let ch = rgb_to_hsv(col[0], col[1], col[2]);
            if ch[1] != 0.0 {
                let rh = rgb_to_hsv(r[0], r[1], r[2]);
                let t = hsv_to_rgb(ch[0], rh[1], rh[2]);
                for i in 0..3 {
                    r[i] = facm * r[i] + fac * t[i];
                }
            }
        }
        "SATURATION" => {
            let rh = rgb_to_hsv(r[0], r[1], r[2]);
            if rh[1] != 0.0 {
                let ch = rgb_to_hsv(col[0], col[1], col[2]);
                let t = hsv_to_rgb(rh[0], facm * rh[1] + fac * ch[1], rh[2]);
                r[..3].copy_from_slice(&t);
            }
        }
        "VALUE" => {
            let rh = rgb_to_hsv(r[0], r[1], r[2]);
            let ch = rgb_to_hsv(col[0], col[1], col[2]);
            let t = hsv_to_rgb(rh[0], rh[1], facm * rh[2] + fac * ch[2]);
            r[..3].copy_from_slice(&t);
        }
        "COLOR" => {
            let ch = rgb_to_hsv(col[0], col[1], col[2]);
            if ch[1] != 0.0 {
                let rh = rgb_to_hsv(r[0], r[1], r[2]);
                let t = hsv_to_rgb(ch[0], ch[1], rh[2]);
                for i in 0..3 {
                    r[i] = facm * r[i] + fac * t[i];
                }
            }
        }
        "SOFT_LIGHT" => each(r, &|x, c| {
            let scr = 1.0 - (1.0 - c) * (1.0 - x);
            facm * x + fac * (((1.0 - x) * c * x) + (x * scr))
        }),
        "LINEAR_LIGHT" => each(r, &|x, c| {
            if c > 0.5 {
                x + fac * (2.0 * (c - 0.5))
            } else {
                x + fac * (2.0 * c - 1.0)
            }
        }),
        _ => return None,
    }
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::graph::testkit::*;
    use frap_rna::Value as V;

    fn near(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }

    /// Probe fold.py (verifier) and fold2.py, exported by Blender 5.2.4.
    #[test]
    fn folds_like_blender() {
        let images = Images::default();
        // fold.py: Mix RGBA of constants → Base Color (0.5, 0, 0.5); Map Range of 0.25 →
        // Roughness 0.25; a muted Math whose first input is unlinked leaves Metallic dangling
        // (its own value, 0); a Value 1 on Alpha → OPAQUE.
        let mut m = mat("Fold");
        let mix = add(&mut m, "ShaderNodeMix");
        set_enum(&mut m, mix, "data_type", "RGBA");
        set_in(&mut m, mix, "Factor", float(0.5));
        set_in(&mut m, mix, "A", color(1.0, 0.0, 0.0));
        set_in(&mut m, mix, "B", color(0.0, 0.0, 1.0));
        link(&mut m, (mix, "Result"), (BSDF, "Base Color"));
        let mr = add(&mut m, "ShaderNodeMapRange");
        set_in(&mut m, mr, "Value", float(0.25));
        link(&mut m, (mr, "Result"), (BSDF, "Roughness"));
        let math = add(&mut m, "ShaderNodeMath");
        set_enum(&mut m, math, "operation", "MULTIPLY");
        set_in_at(&mut m, math, 0, float(0.5));
        set_in_at(&mut m, math, 1, float(0.8));
        m.node_tree.node_mut(math).unwrap().mute = true;
        link_at(&mut m, (math, 0), (BSDF, 1));
        let lm = lower_m(&m, &images);
        assert_eq!(lm.base_color.factor, [0.5, 0.0, 0.5, 1.0]);
        assert_eq!((lm.roughness.factor, lm.metallic.factor), (0.25, 0.0));
        assert!(lm.diagnostics.is_empty(), "{:?}", lm.diagnostics);

        // fold2.py "A": Clamp RANGE (0.9 in [0.7, 0.2]) → 0.7; Mix (Legacy) SCREEN → (0.48,
        // 0.442, 0.289); Vector Math DOT_PRODUCT → 0.35.
        let mut m = mat("A");
        let c = add(&mut m, "ShaderNodeClamp");
        set_enum(&mut m, c, "clamp_type", "RANGE");
        set_in(&mut m, c, "Value", float(0.9));
        set_in(&mut m, c, "Min", float(0.7));
        set_in(&mut m, c, "Max", float(0.2));
        link(&mut m, (c, "Result"), (BSDF, "Roughness"));
        let x = add(&mut m, "ShaderNodeMixRGB");
        set_enum(&mut m, x, "blend_type", "SCREEN");
        set_in_at(&mut m, x, 0, float(0.7));
        set_in_at(&mut m, x, 1, color(0.2, 0.4, 0.1));
        set_in_at(&mut m, x, 2, color(0.5, 0.1, 0.3));
        link(&mut m, (x, "Color"), (BSDF, "Base Color"));
        let vm = add(&mut m, "ShaderNodeVectorMath");
        set_enum(&mut m, vm, "operation", "DOT_PRODUCT");
        set_in_at(&mut m, vm, 0, SocketValue::Vector([0.1, 0.2, 0.3, 0.0]));
        set_in_at(&mut m, vm, 1, SocketValue::Vector([1.0, 0.5, 0.5, 0.0]));
        link(&mut m, (vm, "Value"), (BSDF, "Metallic"));
        let lm = lower_m(&m, &images);
        let bc = lm.base_color.factor;
        assert!(
            near(bc[0], 0.48) && near(bc[1], 0.442) && near(bc[2], 0.289),
            "{bc:?}"
        );
        assert!(near(lm.roughness.factor, 0.7) && near(lm.metallic.factor, 0.35));

        // fold2.py "B": Combine XYZ → Separate XYZ → Map Range SMOOTHSTEP → 0.648; Gamma 2.2
        // → (0.2176, 0.0474, 0.6121); a muted Math passes its linked second input (Value
        // 0.35); Vector Math SCALE into Emission Color → (0.1, 0.2, 0.3).
        let mut m = mat("B");
        let cx = add(&mut m, "ShaderNodeCombineXYZ");
        set_in(&mut m, cx, "Y", float(0.3));
        let sx = add(&mut m, "ShaderNodeSeparateXYZ");
        link(&mut m, (cx, "Vector"), (sx, "Vector"));
        let mr = add(&mut m, "ShaderNodeMapRange");
        set_enum(&mut m, mr, "interpolation_type", "SMOOTHSTEP");
        set_in(&mut m, mr, "From Max", float(0.5));
        link(&mut m, (sx, "Y"), (mr, "Value"));
        link(&mut m, (mr, "Result"), (BSDF, "Roughness"));
        let g = add(&mut m, "ShaderNodeGamma");
        set_in(&mut m, g, "Color", color(0.5, 0.25, 0.8));
        set_in(&mut m, g, "Gamma", float(2.2));
        link(&mut m, (g, "Color"), (BSDF, "Base Color"));
        let v = add(&mut m, "ShaderNodeValue");
        m.node_tree.node_mut(v).unwrap().outputs[0].default_value = float(0.35);
        let mm = add(&mut m, "ShaderNodeMath");
        set_enum(&mut m, mm, "operation", "MULTIPLY");
        set_in_at(&mut m, mm, 1, float(0.1));
        m.node_tree.node_mut(mm).unwrap().mute = true;
        link_at(&mut m, (v, 0), (mm, 1));
        link_at(&mut m, (mm, 0), (BSDF, 1));
        let vs = add(&mut m, "ShaderNodeVectorMath");
        set_enum(&mut m, vs, "operation", "SCALE");
        set_in_at(&mut m, vs, 0, SocketValue::Vector([0.2, 0.4, 0.6, 0.0]));
        set_in(&mut m, vs, "Scale", float(0.5));
        link(&mut m, (vs, "Vector"), (BSDF, "Emission Color"));
        set_in(&mut m, BSDF, "Emission Strength", float(1.0));
        let lm = lower_m(&m, &images);
        assert!(near(lm.roughness.factor, 0.648), "{}", lm.roughness.factor);
        let bc = lm.base_color.factor;
        assert!(
            near(bc[0], 0.217_637_63) && near(bc[1], 0.047_366_14) && near(bc[2], 0.612_065_6),
            "{bc:?}"
        );
        assert!(near(lm.metallic.factor, 0.35));
        let e = lm.emissive.factor;
        assert!(
            near(e[0], 0.1) && near(e[1], 0.2) && near(e[2], 0.3),
            "{e:?}"
        );

        // fold2.py "C": Mix FLOAT with a clamped factor 1.5 → 0.6; Mix RGBA HUE → (0.44, 0.33,
        // 0.56); a colour into Metallic is its luminance.
        let mut m = mat("C");
        let mf = add(&mut m, "ShaderNodeMix");
        assert!(
            m.node_tree
                .node_mut(mf)
                .unwrap()
                .set_prop("clamp_factor", V::Bool(true))
        );
        set_in(&mut m, mf, "Factor", float(1.5));
        set_in(&mut m, mf, "A", float(0.2));
        set_in(&mut m, mf, "B", float(0.6));
        link(&mut m, (mf, "Result"), (BSDF, "Roughness"));
        let mh = add(&mut m, "ShaderNodeMix");
        set_enum(&mut m, mh, "data_type", "RGBA");
        set_enum(&mut m, mh, "blend_type", "HUE");
        set_in(&mut m, mh, "Factor", float(0.6));
        set_in(&mut m, mh, "A", color(0.8, 0.3, 0.2));
        set_in(&mut m, mh, "B", color(0.1, 0.3, 0.9));
        link(&mut m, (mh, "Result"), (BSDF, "Base Color"));
        let mc = add(&mut m, "ShaderNodeMix");
        set_enum(&mut m, mc, "data_type", "RGBA");
        set_in(&mut m, mc, "Factor", float(1.0));
        set_in(&mut m, mc, "B", color(0.9, 0.5, 0.1));
        link(&mut m, (mc, "Result"), (BSDF, "Metallic"));
        let lm = lower_m(&m, &images);
        assert!(near(lm.roughness.factor, 0.6));
        let bc = lm.base_color.factor;
        assert!(
            near(bc[0], 0.44) && near(bc[1], 0.33) && near(bc[2], 0.56),
            "{bc:?}"
        );
        let lum = 0.2126 * 0.9 + 0.7152 * 0.5 + 0.0722 * 0.1;
        assert!(near(lm.metallic.factor, lum), "{}", lm.metallic.factor);
    }

    /// Cycles: Blender flags the closing link invalid and exports the rest (probe cyc.py:
    /// no texture, no factor); frap must not recurse forever.
    #[test]
    fn cycles_end_the_walks() {
        let mut images = Images::default();
        let img = images.add("I", false);
        let mut m = mat("Cycle");
        let a = add(&mut m, "ShaderNodeMath");
        let b = add(&mut m, "ShaderNodeMath");
        link_at(&mut m, (a, 0), (BSDF, 0));
        link_at(&mut m, (b, 0), (a, 0));
        link_at(&mut m, (a, 0), (b, 0));
        let t = image_node(&mut m, img);
        link_at(&mut m, (t, 0), (b, 1));
        let r1 = add(&mut m, "NodeReroute");
        let r2 = add(&mut m, "NodeReroute");
        link_at(&mut m, (r1, 0), (r2, 0));
        link_at(&mut m, (r2, 0), (r1, 0));
        let alpha = m
            .node_tree
            .node(BSDF)
            .unwrap()
            .input_index("Alpha")
            .unwrap();
        link_at(&mut m, (r1, 0), (BSDF, alpha));
        let lm = lower_m(&m, &images);
        assert!(lm.base_color.texture.is_none());
        assert_eq!(lm.base_color.factor, [1.0; 4]);
        // Blender's inliner gives up on a cyclic tree: the glTF default material.
        assert_eq!((lm.metallic.factor, lm.roughness.factor), (1.0, 1.0));
        assert!(
            lm.diagnostics[0].message.contains("cycle"),
            "{:?}",
            lm.diagnostics
        );
        // With a "glTF Material Output" Occlusion input the original tree is read: the
        // searches walk the cycle and end (the Image Texture's link is invalid).
        settings_node(&mut m);
        let lm = lower_m(&m, &images);
        assert!(lm.base_color.texture.is_none());
        assert_eq!(lm.metallic.factor, 0.0);
        // Separate Color nodes in a loop.
        let mut m = mat("Sep");
        let s1 = add(&mut m, "ShaderNodeSeparateColor");
        let s2 = add(&mut m, "ShaderNodeSeparateColor");
        link_at(&mut m, (s1, 0), (s2, 0));
        link_at(&mut m, (s2, 0), (s1, 0));
        link_at(&mut m, (s1, 0), (BSDF, 1));
        lower_m(&m, &images);
    }

    #[test]
    fn ramp_blend_modes() {
        let mut r = [1.0, 0.0, 0.0, 1.0];
        ramp_blend("MIX", &mut r, 0.5, &[0.0, 0.0, 1.0, 1.0]).unwrap();
        assert_eq!(r, [0.5, 0.0, 0.5, 1.0]);
        let mut r = [0.5, 0.5, 0.5, 1.0];
        ramp_blend("MULTIPLY", &mut r, 1.0, &[0.5, 1.0, 0.0, 1.0]).unwrap();
        assert_eq!(r, [0.25, 0.5, 0.0, 1.0]);
        let mut r = [0.2, 0.4, 0.6, 1.0];
        ramp_blend("VALUE", &mut r, 1.0, &[1.0, 1.0, 1.0, 1.0]).unwrap();
        assert!((r[2] - 1.0).abs() < 1e-6, "{r:?}");
        assert!(ramp_blend("NOPE", &mut r, 1.0, &[0.0; 4]).is_none());
    }

    #[test]
    fn conversions() {
        assert_eq!(FoldVal::Vector([1.0, 2.0, 3.0]).f(), 2.0);
        assert_eq!(FoldVal::Float(0.5).c(), [0.5, 0.5, 0.5, 1.0]);
        let l = FoldVal::Color([1.0, 1.0, 1.0, 0.2]).f();
        assert!((l - 1.0).abs() < 1e-6);
        assert_eq!(link_priority(SocketType::Value, SocketType::Value), 5);
        assert_eq!(link_priority(SocketType::Rgba, SocketType::Value), 2);
        assert_eq!(link_priority(SocketType::Shader, SocketType::Shader), 1);
        assert_eq!(link_priority(SocketType::Shader, SocketType::Rgba), -1);
    }
}
