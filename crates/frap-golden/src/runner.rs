// SPDX-License-Identifier: GPL-3.0-or-later
//! frap side: run a case through an in-process `Session` and dump through frap's RNA (`tests/golden/props.toml`). WP 1.8.
//!
//! The dump reads every property of `props.toml` with ordinary statements executed by the
//! session (`bpy.data.objects['Cube'].matrix_world`, `….vertices.foreach_get('co')`), exactly
//! as an agent would, so the golden suite tests the RNA layer and its hooks too. A datablock
//! whose RNA read fails (a binding that has not landed yet) is dumped natively from `Main`
//! instead, and the fallback is recorded in [`FrapRun::rna_fallbacks`]; with
//! [`RunOptions::strict_rna`] (env `FRAP_GOLDEN_STRICT_RNA=1`) a fallback is an error.
//!
//! [`compare`] dispatches a case's `compare=` items to the comparators, and
//! [`check_against_fixture`] is the whole fixture test of one case.

use std::collections::BTreeMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};

use frap_mesh::{AttrData, Domain, Mesh};
use frap_nodes::SocketValue;
use frap_rna::{IdCode, IdNames, IdRef, MeshId, ObjectId};
use frap_scene::Main;
use frap_scene::id::{Object, SlotLink};
use frap_session::{Response, Session, SessionOptions, Verbosity};
use serde::Deserialize;
use serde_json::{Value as J, json};

use crate::blender::{HarnessError, Repo, ScratchDir};
use crate::case::{Case, CompareSpec, Mismatch, Order, Side, Startup, Tier};
use crate::compare::{
    gltf_canonical, image_ssim, invariants, lines, mesh_canonical, mesh_exact, rna_values,
};
use crate::{GAction, GContext, GFile, GMaterial, GMesh, GObject, GoldenState, LineOutcome};

/// Environment variable that makes RNA fallbacks errors.
pub const STRICT_RNA_ENV: &str = "FRAP_GOLDEN_STRICT_RNA";

/// `[object]` of `props.toml`.
#[derive(Clone, Debug, Deserialize)]
#[allow(missing_docs)]
pub struct ObjectProps {
    #[serde(rename = "type")]
    pub r#type: String,
    pub parent: String,
    pub matrix_world: String,
    pub location: String,
    pub rotation_mode: String,
    pub scale: String,
    pub mode: String,
    pub modifiers: String,
    pub modifier_props: Vec<String>,
    pub materials: String,
    /// rotation property per `rotation_mode` (`QUATERNION`, `AXIS_ANGLE`, `EULER`).
    pub rotation: BTreeMap<String, String>,
}

/// `[mesh]` of `props.toml`.
#[derive(Clone, Debug, Deserialize)]
#[allow(missing_docs)]
pub struct MeshProps {
    pub positions: String,
    pub edges: String,
    pub faces: String,
    pub corner_normals: String,
    pub uv_layers: String,
    pub uv_value: String,
    pub attributes: String,
    pub attribute_props: Vec<String>,
    pub selection: Vec<String>,
    /// `data` value field per attribute `data_type`.
    pub attribute_value: BTreeMap<String, String>,
}

/// `[context]` of `props.toml`.
#[derive(Clone, Debug, Deserialize)]
#[allow(missing_docs)]
pub struct ContextProps {
    pub active: String,
    pub selected: String,
    pub mode: String,
    pub select_mode: String,
    pub cursor: String,
}

/// `[material]` of `props.toml`.
#[derive(Clone, Debug, Deserialize)]
#[allow(missing_docs)]
pub struct MaterialProps {
    pub nodes: String,
    pub node_props: Vec<String>,
    pub inputs: String,
    pub links: String,
}

/// `[derived]` of `props.toml`.
#[derive(Clone, Debug, Deserialize)]
pub struct DerivedProps {
    /// Property and function names whose reads are derived values (D-04).
    pub props: Vec<String>,
}

/// `tests/golden/props.toml`: the property list both dumpers read.
#[derive(Clone, Debug, Deserialize)]
#[allow(missing_docs)]
pub struct Props {
    pub version: u32,
    pub object: ObjectProps,
    pub mesh: MeshProps,
    pub context: ContextProps,
    pub material: MaterialProps,
    pub derived: DerivedProps,
}

impl Props {
    /// Parses `props.toml`.
    pub fn parse(text: &str) -> Result<Props, HarnessError> {
        let p: Props =
            toml::from_str(text).map_err(|e| HarnessError::Json(format!("props.toml: {e}")))?;
        if p.version != 1 {
            return Err(HarnessError::Json(format!(
                "props.toml: unsupported version {}",
                p.version
            )));
        }
        for k in ["QUATERNION", "AXIS_ANGLE", "EULER"] {
            if !p.object.rotation.contains_key(k) {
                return Err(HarnessError::Json(format!(
                    "props.toml: [object.rotation] lacks {k}"
                )));
            }
        }
        Ok(p)
    }
    /// Reads and parses a file.
    pub fn load(path: &Path) -> Result<Props, HarnessError> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| HarnessError::Io(format!("cannot read {}: {e}", path.display())))?;
        Props::parse(&text)
    }
}

/// Options of a frap run.
#[derive(Clone, Debug)]
pub struct RunOptions {
    /// Current directory of the session (relative output paths land here).
    pub workdir: PathBuf,
    /// Fail instead of falling back to native reads.
    pub strict_rna: bool,
}

/// Result of a frap run.
#[derive(Clone, Debug)]
pub struct FrapRun {
    /// The dump (files resolved: glTF canonical, images by absolute path).
    pub state: GoldenState,
    /// Datablocks read natively because an RNA read failed, with the first failing read.
    pub rna_fallbacks: Vec<String>,
}

/// Python string literal.
pub fn py_str(s: &str) -> String {
    format!("'{}'", s.replace('\\', "\\\\").replace('\'', "\\'"))
}

/// `bpy.data.<coll>['<name>']`.
fn data_path(coll: &str, name: &str) -> String {
    format!("bpy.data.{coll}[{}]", py_str(name))
}

/// Replaces the work directory (as given and canonicalised) by `<workdir>` in messages, so
/// both sides compare equal wherever they ran.
pub fn scrub_workdir(lines: &mut [LineOutcome], workdir: &Path) {
    let mut variants = vec![workdir.to_string_lossy().into_owned()];
    if let Ok(c) = workdir.canonicalize() {
        variants.push(c.to_string_lossy().into_owned());
    }
    variants.sort_by_key(|v| std::cmp::Reverse(v.len()));
    for l in lines {
        if let Some(m) = &mut l.message {
            for v in &variants {
                if !v.is_empty() {
                    *m = m.replace(v.as_str(), "<workdir>");
                }
            }
        }
    }
}

// ------------------------------------------------------------------ JSON value parsing

fn f(v: &J) -> Result<f32, String> {
    match v {
        J::Number(n) => n
            .as_f64()
            .map(|x| x as f32)
            .ok_or_else(|| "bad number".into()),
        J::Object(o) => match o.get("$float").and_then(|s| s.as_str()) {
            Some("nan") => Ok(f32::NAN),
            Some("inf") => Ok(f32::INFINITY),
            Some("-inf") => Ok(f32::NEG_INFINITY),
            _ => Err(format!("not a float: {v}")),
        },
        J::Bool(b) => Ok(*b as u8 as f32),
        _ => Err(format!("not a float: {v}")),
    }
}

fn floats(v: &J) -> Result<Vec<f32>, String> {
    v.as_array()
        .ok_or_else(|| format!("not an array: {v}"))?
        .iter()
        .map(f)
        .collect()
}

fn fixed<const N: usize>(v: &J) -> Result<[f32; N], String> {
    let x = floats(v)?;
    x.try_into()
        .map_err(|x: Vec<f32>| format!("expected {N} floats, got {}", x.len()))
}

fn s(v: &J) -> Result<String, String> {
    v.as_str()
        .map(str::to_owned)
        .ok_or_else(|| format!("not a string: {v}"))
}

/// The name in `{"$ref": "bpy.data.objects['Cube']"}` (`None` for `null`).
fn ref_name(v: &J) -> Result<Option<String>, String> {
    if v.is_null() {
        return Ok(None);
    }
    let r = v
        .get("$ref")
        .and_then(|r| r.as_str())
        .ok_or_else(|| format!("not an ID reference: {v}"))?;
    let start = r.find("['").or_else(|| r.find("[\"")).ok_or("bad $ref")? + 2;
    let end = r.rfind("']").or_else(|| r.rfind("\"]")).ok_or("bad $ref")?;
    Ok(Some(
        r.get(start..end)
            .ok_or("bad $ref")?
            .replace("\\'", "'")
            .replace("\\\\", "\\"),
    ))
}

fn coll_len(v: &J) -> Result<usize, String> {
    v.get("len")
        .and_then(|l| l.as_u64())
        .map(|l| l as usize)
        .ok_or_else(|| format!("not a collection: {v}"))
}

fn coll_items(v: &J) -> Result<Vec<String>, String> {
    if v.get("truncated").and_then(|t| t.as_bool()) == Some(true) {
        return Err("collection names truncated".into());
    }
    v.get("items")
        .and_then(|i| i.as_array())
        .ok_or_else(|| format!("collection without names: {v}"))?
        .iter()
        .map(s)
        .collect()
}

/// Splits a `$array` bulk value into one JSON value per item.
fn bulk_items(v: &J) -> Result<Vec<J>, String> {
    let data = v
        .get("$array")
        .and_then(|a| a.as_array())
        .ok_or_else(|| format!("not a bulk array: {v}"))?;
    if v.get("truncated").and_then(|t| t.as_bool()) == Some(true) {
        return Err("bulk array truncated".into());
    }
    let shape: Vec<usize> = v
        .get("shape")
        .and_then(|s| s.as_array())
        .map(|s| {
            s.iter()
                .filter_map(|x| x.as_u64())
                .map(|x| x as usize)
                .collect()
        })
        .unwrap_or_default();
    match shape.as_slice() {
        [_] | [] => Ok(data.clone()),
        [n, k] if n * k == data.len() && *k > 0 => {
            Ok(data.chunks(*k).map(|c| J::Array(c.to_vec())).collect())
        }
        _ => Err(format!("unexpected bulk shape {shape:?}")),
    }
}

// ------------------------------------------------------------------ native math (fallback)

type M4 = [[f64; 4]; 4];

fn mul(a: &M4, b: &M4) -> M4 {
    let mut r = [[0.0; 4]; 4];
    for (i, row) in r.iter_mut().enumerate() {
        for (j, x) in row.iter_mut().enumerate() {
            *x = (0..4).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    r
}

fn ident() -> M4 {
    let mut m = [[0.0; 4]; 4];
    for (i, row) in m.iter_mut().enumerate() {
        row[i] = 1.0;
    }
    m
}

fn rot_axis(axis: usize, a: f64) -> M4 {
    let (s, c) = a.sin_cos();
    let mut m = ident();
    let (i, j) = match axis {
        0 => (1, 2),
        1 => (2, 0),
        _ => (0, 1),
    };
    m[i][i] = c;
    m[i][j] = -s;
    m[j][i] = s;
    m[j][j] = c;
    m
}

/// Blender `eulO_to_mat3`: order `ABC` applies A first: `R = R_C · R_B · R_A`.
fn euler(order: &str, e: [f32; 3]) -> M4 {
    let mut m = ident();
    for ch in order.chars() {
        let axis = match ch {
            'X' => 0,
            'Y' => 1,
            _ => 2,
        };
        m = mul(&rot_axis(axis, e[axis] as f64), &m);
    }
    m
}

/// Blender `quat_to_mat3` after `normalize_qt` (w-first; zero quaternion = identity).
fn quat(q: [f32; 4]) -> M4 {
    let q = q.map(|x| x as f64);
    let l = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
    if l == 0.0 {
        return ident();
    }
    let [w, x, y, z] = q.map(|v| v / l);
    let mut m = ident();
    m[0] = [
        1.0 - 2.0 * (y * y + z * z),
        2.0 * (x * y - w * z),
        2.0 * (x * z + w * y),
        0.0,
    ];
    m[1] = [
        2.0 * (x * y + w * z),
        1.0 - 2.0 * (x * x + z * z),
        2.0 * (y * z - w * x),
        0.0,
    ];
    m[2] = [
        2.0 * (x * z - w * y),
        2.0 * (y * z + w * x),
        1.0 - 2.0 * (x * x + y * y),
        0.0,
    ];
    m
}

/// Blender `axis_angle_to_mat3` (angle, x, y, z); a zero axis gives identity.
fn axis_angle(aa: [f32; 4]) -> M4 {
    let ang = aa[0] as f64;
    let l = ((aa[1] as f64).powi(2) + (aa[2] as f64).powi(2) + (aa[3] as f64).powi(2)).sqrt();
    if l == 0.0 {
        return ident();
    }
    let (s, c) = (ang.sin(), ang.cos());
    let (x, y, z) = (aa[1] as f64 / l, aa[2] as f64 / l, aa[3] as f64 / l);
    let t = 1.0 - c;
    let mut m = ident();
    m[0] = [t * x * x + c, t * x * y - s * z, t * x * z + s * y, 0.0];
    m[1] = [t * x * y + s * z, t * y * y + c, t * y * z - s * x, 0.0];
    m[2] = [t * x * z - s * y, t * y * z + s * x, t * z * z + c, 0.0];
    m
}

/// `BKE_object_to_mat4`: `T(loc + dloc) · (Rdelta · R) · S(scale · dscale)`.
fn local_matrix(ob: &Object) -> M4 {
    let (r, dr) = match ob.rotation_mode.as_str() {
        "QUATERNION" => (
            quat(ob.rotation_quaternion),
            quat(ob.delta_rotation_quaternion),
        ),
        "AXIS_ANGLE" => (axis_angle(ob.rotation_axis_angle), ident()),
        order => (
            euler(order, ob.rotation_euler),
            euler(order, ob.delta_rotation_euler),
        ),
    };
    let rot = mul(&dr, &r);
    let mut s = ident();
    for (i, row) in s.iter_mut().enumerate().take(3) {
        row[i] = ob.scale[i] as f64 * ob.delta_scale[i] as f64;
    }
    let mut m = mul(&rot, &s);
    for (i, row) in m.iter_mut().enumerate().take(3) {
        row[3] = ob.location[i] as f64 + ob.delta_location[i] as f64;
    }
    m
}

fn world_matrix(main: &Main, r: IdRef<ObjectId>, depth: usize) -> M4 {
    let Some(ob) = main.objects.get(r) else {
        return ident();
    };
    let local = local_matrix(ob);
    match ob.parent {
        Some(p) if depth < 128 => {
            let pw = world_matrix(main, p, depth + 1);
            let cols = ob.matrix_parent_inverse.to_cols_array();
            let mut inv = [[0.0; 4]; 4];
            for (c, col) in cols.chunks(4).enumerate() {
                for (row, v) in col.iter().enumerate() {
                    inv[row][c] = *v as f64;
                }
            }
            mul(&mul(&pw, &inv), &local)
        }
        _ => local,
    }
}

fn srgb_to_linear(c: u8) -> f32 {
    let v = c as f32 / 255.0;
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

fn domain_str(d: Domain) -> &'static str {
    match d {
        Domain::Point => "POINT",
        Domain::Edge => "EDGE",
        Domain::Face => "FACE",
        Domain::Corner => "CORNER",
    }
}

fn fj(x: f32) -> J {
    frap_session::response::f32_json(x)
}

fn fjs(v: &[f32]) -> J {
    J::Array(v.iter().map(|x| fj(*x)).collect())
}

fn attr_json(data: &AttrData) -> Option<J> {
    Some(J::Array(match data {
        AttrData::Float(v) => v.iter().map(|x| fj(*x)).collect(),
        AttrData::Int(v) => v.iter().map(|x| J::from(*x)).collect(),
        AttrData::Int8(v) => v.iter().map(|x| J::from(*x)).collect(),
        AttrData::Bool(v) => v.iter().map(|x| J::from(*x)).collect(),
        AttrData::Float2(v) => v.iter().map(|x| fjs(x)).collect(),
        AttrData::Float3(v) => v.iter().map(|x| fjs(x)).collect(),
        AttrData::Float4(v) | AttrData::FloatColor(v) | AttrData::Quaternion(v) => {
            v.iter().map(|x| fjs(x)).collect()
        }
        AttrData::ByteColor(v) => v
            .iter()
            .map(|c| {
                fjs(&[
                    srgb_to_linear(c[0]),
                    srgb_to_linear(c[1]),
                    srgb_to_linear(c[2]),
                    c[3] as f32 / 255.0,
                ])
            })
            .collect(),
        AttrData::Int16x2(v) => v.iter().map(|x| json!([x[0], x[1]])).collect(),
        AttrData::Int32x2(v) => v.iter().map(|x| json!([x[0], x[1]])).collect(),
        AttrData::Float4x4(v) => v
            .iter()
            .map(|m| J::Array(m.iter().map(|r| fjs(r)).collect()))
            .collect(),
        AttrData::String(v) => v.iter().map(|x| J::from(x.as_str())).collect(),
        _ => return None,
    }))
}

/// Native mesh dump (mirrors `dump_state.py::dump_mesh` in object mode).
pub fn native_mesh(m: &Mesh) -> GMesh {
    let uv = m.uv_maps();
    let uv_names: Vec<&str> = uv.iter().map(|(n, _)| *n).collect();
    let mut attributes = BTreeMap::new();
    for a in m.attributes().iter() {
        if a.name.starts_with('.') || a.name == "position" || uv_names.contains(&a.name.as_str()) {
            continue;
        }
        if let Some(data) = attr_json(&a.data) {
            attributes.insert(
                a.name.to_string(),
                json!({"domain": domain_str(a.domain), "data_type": a.data.data_type(), "data": data}),
            );
        }
    }
    GMesh {
        positions: m.positions().to_vec(),
        edges: m
            .edges()
            .iter()
            .map(|e| [e[0] as u32, e[1] as u32])
            .collect(),
        faces: (0..m.faces_num())
            .map(|i| {
                m.corner_verts()[m.face(i)]
                    .iter()
                    .map(|v| *v as u32)
                    .collect()
            })
            .collect(),
        corner_normals: m.corner_normals().to_vec(),
        uv_layers: uv
            .iter()
            .map(|(n, d)| (n.to_string(), d.to_vec()))
            .collect(),
        attributes,
        selection: J::Null,
        active: J::Null,
        select_history: J::Null,
    }
}

fn native_object(main: &Main, r: IdRef<ObjectId>, ob: &Object) -> GObject {
    let mw = world_matrix(main, r, 0);
    let rotation = match ob.rotation_mode.as_str() {
        "QUATERNION" => ob.rotation_quaternion.to_vec(),
        "AXIS_ANGLE" => ob.rotation_axis_angle.to_vec(),
        _ => ob.rotation_euler.to_vec(),
    };
    let data_mats: Vec<Option<String>> = match ob.data {
        Some(d) if d.code == IdCode::ME => IdRef::<MeshId>::from_any(d)
            .and_then(|m| main.meshes.get(m))
            .map(|m| {
                m.materials
                    .iter()
                    .map(|x| x.and_then(|x| main.materials.name_of(x)).map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default(),
        _ => Vec::new(),
    };
    let materials = data_mats
        .into_iter()
        .enumerate()
        .map(|(i, dm)| match ob.material_slot_overrides.get(i) {
            Some(o) if o.link == SlotLink::Object => o
                .material
                .and_then(|x| main.materials.name_of(x))
                .map(str::to_owned),
            _ => dm,
        })
        .collect();
    GObject {
        r#type: ob.object_type().to_owned(),
        parent: ob
            .parent
            .and_then(|p| main.objects.name_of(p))
            .map(str::to_owned),
        matrix_world: mw.map(|row| row.map(|x| x as f32)),
        location: ob.location,
        rotation_mode: ob.rotation_mode.to_string(),
        rotation,
        scale: ob.scale,
        // Modifier types come from the generated settings (M4); M1 objects have none.
        modifiers: ob
            .modifiers
            .iter()
            .map(|m| (m.name.to_string(), String::from("?")))
            .collect(),
        materials,
        mode: ob.mode.as_str().to_owned(),
    }
}

fn socket_json(main: &Main, v: &SocketValue) -> Option<J> {
    Some(match v {
        SocketValue::Float(x) => fj(*x),
        SocketValue::Int(x) => J::from(*x),
        SocketValue::Bool(x) => J::from(*x),
        SocketValue::Vector(x) => fjs(&x[..3]),
        SocketValue::Color(x) => fjs(x),
        SocketValue::Rotation(x) => fjs(x),
        SocketValue::String(x) | SocketValue::Menu(x) => J::from(x.as_str()),
        SocketValue::Id(None) => J::Null,
        SocketValue::Id(Some(id)) => main
            .id_name(*id)
            .map(|(n, _)| J::from(n.as_str()))
            .unwrap_or(J::Null),
        _ => return None,
    })
}

fn native_material(main: &Main, name: &str) -> Option<GMaterial> {
    let r = main.materials.find(name)?;
    let mat = main.materials.get(r)?;
    let tree = &mat.node_tree;
    let mut nodes = serde_json::Map::new();
    for n in &tree.nodes {
        let mut inputs = serde_json::Map::new();
        for sock in &n.inputs {
            if let Some(v) = socket_json(main, &sock.default_value) {
                inputs.insert(sock.identifier.to_string(), v);
            }
        }
        nodes.insert(
            n.name.to_string(),
            json!({"bl_idname": n.bl_idname, "mute": n.mute, "inputs": inputs}),
        );
    }
    let node_name = |uid: u32| {
        tree.nodes
            .iter()
            .find(|n| n.uid == uid)
            .map(|n| n.name.to_string())
            .unwrap_or_default()
    };
    let mut links: Vec<[String; 4]> = tree
        .links
        .iter()
        .map(|l| {
            [
                node_name(l.from_node),
                l.from_socket.to_string(),
                node_name(l.to_node),
                l.to_socket.to_string(),
            ]
        })
        .collect();
    links.sort();
    Some(GMaterial {
        nodes: J::Object(nodes),
        links: json!(links),
    })
}

// ------------------------------------------------------------------ the dumper

struct Dumper<'a> {
    session: Session,
    props: &'a Props,
    fallbacks: Vec<String>,
    context: Option<J>,
}

impl Dumper<'_> {
    /// Runs one read statement; the value of a FINISHED response.
    fn read(&mut self, expr: &str) -> Result<J, String> {
        let session = &mut self.session;
        let out = catch_unwind(AssertUnwindSafe(|| session.exec_source(expr)))
            .map_err(|_| format!("{expr}: panicked"))?;
        let [r]: [Response; 1] = out
            .try_into()
            .map_err(|v: Vec<Response>| format!("{expr}: {} responses", v.len()))?;
        if let Some(c) = &r.context {
            self.context = Some(c.clone());
        }
        if r.ok() {
            r.value.ok_or_else(|| format!("{expr}: no value"))
        } else {
            let e = r.error.as_ref();
            Err(format!(
                "{expr}: {} {}",
                e.map_or("CANCELLED", |e| e.code.0),
                e.map_or("", |e| e.message.as_str())
            ))
        }
    }

    /// `coll/prop` relative to `base`: one value per item (bulk `foreach_get` first, item by
    /// item otherwise).
    fn read_items(&mut self, base: &str, path: &str) -> Result<Vec<J>, String> {
        let (coll, prop) = path.split_once('/').ok_or("not a coll/prop path")?;
        let prop = prop.replace('/', ".");
        let coll_path = format!("{base}.{}", coll.replace('/', "."));
        let n = coll_len(&self.read(&coll_path)?)?;
        if !prop.contains('.')
            && let Ok(v) = self.read(&format!("{coll_path}.foreach_get({})", py_str(&prop)))
            && let Ok(items) = bulk_items(&v)
            && items.len() == n
        {
            return Ok(items);
        }
        (0..n)
            .map(|i| self.read(&format!("{coll_path}[{i}].{prop}")))
            .collect()
    }

    fn fallback(&mut self, what: String) {
        self.fallbacks.push(what);
    }

    fn rna_object(&mut self, name: &str) -> Result<GObject, String> {
        let p = &self.props.object;
        let (ty, parent, mw, loc, rm, scale, mode, mods, mats, rot) = (
            p.r#type.clone(),
            p.parent.clone(),
            p.matrix_world.clone(),
            p.location.clone(),
            p.rotation_mode.clone(),
            p.scale.clone(),
            p.mode.clone(),
            p.modifiers.clone(),
            p.materials.clone(),
            p.rotation.clone(),
        );
        let mprops = p.modifier_props.clone();
        let base = data_path("objects", name);
        let rotation_mode = s(&self.read(&format!("{base}.{rm}"))?)?;
        let rot_prop = match rotation_mode.as_str() {
            "QUATERNION" | "AXIS_ANGLE" => &rot[&rotation_mode],
            _ => &rot["EULER"],
        };
        let mwj = self.read(&format!("{base}.{mw}"))?;
        let rows = mwj.as_array().ok_or("matrix_world is not an array")?;
        if rows.len() != 4 {
            return Err("matrix_world is not 4x4".into());
        }
        let mut matrix_world = [[0.0f32; 4]; 4];
        for (i, r) in rows.iter().enumerate() {
            matrix_world[i] = fixed::<4>(r)?;
        }
        let nmods = coll_len(&self.read(&format!("{base}.{mods}"))?)?;
        let mut modifiers = Vec::new();
        for i in 0..nmods {
            let mut v = Vec::new();
            for k in &mprops {
                v.push(s(&self.read(&format!("{base}.{mods}[{i}].{k}"))?)?);
            }
            if v.len() != 2 {
                return Err("modifier_props must name two properties".into());
            }
            modifiers.push((v[0].clone(), v[1].clone()));
        }
        let materials = self
            .read_items(&base, &mats)?
            .iter()
            .map(ref_name)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(GObject {
            r#type: s(&self.read(&format!("{base}.{ty}"))?)?,
            parent: ref_name(&self.read(&format!("{base}.{parent}"))?)?,
            matrix_world,
            location: fixed::<3>(&self.read(&format!("{base}.{loc}"))?)?,
            rotation: floats(&self.read(&format!("{base}.{rot_prop}"))?)?,
            rotation_mode,
            scale: fixed::<3>(&self.read(&format!("{base}.{scale}"))?)?,
            modifiers,
            materials,
            mode: s(&self.read(&format!("{base}.{mode}"))?)?,
        })
    }

    fn rna_mesh(&mut self, base: &str) -> Result<GMesh, String> {
        let p = self.props.mesh.clone();
        let to = |v: Vec<J>, n: usize| -> Result<Vec<Vec<f32>>, String> {
            v.iter()
                .map(|x| {
                    let f = floats(x)?;
                    if f.len() == n {
                        Ok(f)
                    } else {
                        Err(format!("expected {n} components"))
                    }
                })
                .collect()
        };
        let positions = to(self.read_items(base, &p.positions)?, 3)?
            .into_iter()
            .map(|v| [v[0], v[1], v[2]])
            .collect();
        let edges = to(self.read_items(base, &p.edges)?, 2)?
            .into_iter()
            .map(|v| [v[0] as u32, v[1] as u32])
            .collect();
        let faces = self
            .read_items(base, &p.faces)?
            .iter()
            .map(|x| Ok(floats(x)?.into_iter().map(|i| i as u32).collect()))
            .collect::<Result<Vec<Vec<u32>>, String>>()?;
        let corner_normals = to(self.read_items(base, &p.corner_normals)?, 3)?
            .into_iter()
            .map(|v| [v[0], v[1], v[2]])
            .collect();
        let uv_coll = format!("{base}.{}", p.uv_layers);
        let uv_names = coll_items(&self.read(&uv_coll)?)?;
        let mut uv_layers = BTreeMap::new();
        for n in &uv_names {
            let lb = format!("{uv_coll}[{}]", py_str(n));
            let uv = to(self.read_items(&lb, &p.uv_value)?, 2)?
                .into_iter()
                .map(|v| [v[0], v[1]])
                .collect();
            uv_layers.insert(n.clone(), uv);
        }
        let attr_coll = format!("{base}.{}", p.attributes);
        let mut attributes = BTreeMap::new();
        for n in coll_items(&self.read(&attr_coll)?)? {
            if n.starts_with('.') || n == "position" || uv_names.contains(&n) {
                continue;
            }
            let ab = format!("{attr_coll}[{}]", py_str(&n));
            let mut entry = serde_json::Map::new();
            for k in &p.attribute_props {
                entry.insert(k.clone(), self.read(&format!("{ab}.{k}"))?);
            }
            let dt = entry
                .get("data_type")
                .and_then(|d| d.as_str())
                .ok_or("attribute without data_type")?
                .to_owned();
            let field = p
                .attribute_value
                .get(&dt)
                .ok_or_else(|| format!("no value field for {dt}"))?;
            let data = self.read_items(&ab, &format!("data/{field}"))?;
            entry.insert("data".into(), J::Array(data));
            attributes.insert(n, J::Object(entry));
        }
        Ok(GMesh {
            positions,
            edges,
            faces,
            corner_normals,
            uv_layers,
            attributes,
            selection: J::Null,
            active: J::Null,
            select_history: J::Null,
        })
    }

    fn rna_context(&mut self) -> Result<GContext, String> {
        let p = self.props.context.clone();
        let active = ref_name(&self.read(&format!("bpy.context.{}", p.active))?)?;
        let sel = self.read(&format!("bpy.context.{}", p.selected))?;
        let selected = match sel.as_array() {
            Some(a) => a
                .iter()
                .map(|x| ref_name(x)?.ok_or_else(|| "null in selected_objects".to_string()))
                .collect::<Result<Vec<_>, _>>()?,
            None => coll_items(&sel)?,
        };
        let mode = s(&self.read(&format!("bpy.context.{}", p.mode))?)?;
        let select_mode = if mode == "EDIT_MESH" {
            let v = self.read(&format!("bpy.context.{}", p.select_mode))?;
            let b: Vec<bool> = v
                .as_array()
                .ok_or("select_mode is not an array")?
                .iter()
                .map(|x| x.as_bool().ok_or("not a bool"))
                .collect::<Result<_, _>>()?;
            Some(b.try_into().map_err(|_| "select_mode needs 3 bools")?)
        } else {
            None
        };
        Ok(GContext {
            active,
            selected,
            mode,
            select_mode,
            cursor: fixed::<3>(&self.read(&format!("bpy.context.{}", p.cursor))?)?,
        })
    }

    fn native_context(&mut self) -> GContext {
        if self.context.is_none() {
            let _ = self.read("bpy.data");
        }
        let c = self.context.clone().unwrap_or(J::Null);
        let main = self.session.main();
        let scene = main.scenes.iter().next().map(|(_, s)| s);
        let mode = c["mode"].as_str().unwrap_or("OBJECT").to_owned();
        GContext {
            active: c["active_object"].as_str().map(str::to_owned),
            selected: c["selected_objects"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default(),
            select_mode: (mode == "EDIT_MESH")
                .then(|| scene.map(|s| s.tool_settings.mesh_select_mode))
                .flatten(),
            mode,
            cursor: scene.map(|s| s.cursor.location).unwrap_or([0.0; 3]),
        }
    }

    fn rna_material(&mut self, name: &str) -> Result<GMaterial, String> {
        let p = self.props.material.clone();
        let base = data_path("materials", name);
        let nodes_path = format!("{base}.{}", p.nodes);
        let (sock_coll, sock_prop) = p.inputs.split_once('/').ok_or("bad inputs path")?;
        let mut nodes = serde_json::Map::new();
        for n in coll_items(&self.read(&nodes_path)?)? {
            let nb = format!("{nodes_path}[{}]", py_str(&n));
            let mut entry = serde_json::Map::new();
            for k in &p.node_props {
                entry.insert(k.clone(), self.read(&format!("{nb}.{k}"))?);
            }
            let sc = format!("{nb}.{sock_coll}");
            let count = coll_len(&self.read(&sc)?)?;
            let mut inputs = serde_json::Map::new();
            for i in 0..count {
                let id = s(&self.read(&format!("{sc}[{i}].identifier"))?)?;
                match self.read(&format!("{sc}[{i}].{sock_prop}")) {
                    Ok(v) => {
                        let v = match ref_name(&v) {
                            Ok(Some(n)) if v.get("$ref").is_some() => J::from(n),
                            _ => v,
                        };
                        inputs.insert(id, v);
                    }
                    // Sockets without a value (shader, geometry) have no attribute.
                    Err(e) if e.contains("NO_ATTRIBUTE") => {}
                    Err(e) => return Err(e),
                }
            }
            entry.insert("inputs".into(), J::Object(inputs));
            nodes.insert(n, J::Object(entry));
        }
        let lp = format!("{base}.{}", p.links);
        let mut links = Vec::new();
        for i in 0..coll_len(&self.read(&lp)?)? {
            let mut l = Vec::new();
            for k in [
                "from_node.name",
                "from_socket.identifier",
                "to_node.name",
                "to_socket.identifier",
            ] {
                l.push(s(&self.read(&format!("{lp}[{i}].{k}"))?)?);
            }
            links.push(l);
        }
        links.sort();
        Ok(GMaterial {
            nodes: J::Object(nodes),
            links: json!(links),
        })
    }
}

/// Runs `case` through frap and dumps the state (§20.3).
pub fn run_frap(case: &Case, props: &Props, opts: &RunOptions) -> Result<FrapRun, HarnessError> {
    let mut so = SessionOptions::default();
    so.cwd = opts.workdir.clone();
    so.startup = match case.startup {
        Startup::Factory => frap_session::Startup::Factory,
        Startup::Empty => frap_session::Startup::Empty,
    };
    so.verbosity = Verbosity::Normal;
    so.max_items = usize::MAX;
    so.max_array = usize::MAX;
    let session = Session::new(so)
        .map_err(|e| HarnessError::Frap(format!("cannot start a session: {}", e.message)))?;
    let mut d = Dumper {
        session,
        props,
        fallbacks: Vec::new(),
        context: None,
    };

    // 1. The statements.
    let src = case.source_for(Side::Frap);
    let session = &mut d.session;
    let responses = catch_unwind(AssertUnwindSafe(|| session.exec_source_at(&src, 1)))
        .map_err(|_| HarnessError::Frap(format!("{}: frap panicked", case.id())))?;
    let mut line_outcomes: Vec<LineOutcome> = responses
        .iter()
        .map(|r| LineOutcome {
            line: r.line,
            ok: r.ok(),
            exception: r.error.as_ref().map(|e| e.exception.as_str().to_owned()),
            message: r.error.as_ref().map(|e| e.message.clone()),
        })
        .collect();
    scrub_workdir(&mut line_outcomes, &opts.workdir);

    // 2. The dump.
    let object_names: Vec<String> = d
        .session
        .main()
        .objects
        .iter()
        .map(|(_, o)| o.header.name.to_string())
        .collect();
    let mut objects = BTreeMap::new();
    for name in object_names {
        let o = match d.rna_object(&name) {
            Ok(o) => o,
            Err(why) => {
                d.fallback(format!("objects[{name:?}]: {why}"));
                let main = d.session.main();
                let r = main.objects.find(&name).expect("invariant: listed object");
                native_object(
                    main,
                    r,
                    main.objects.get(r).expect("invariant: listed object"),
                )
            }
        };
        objects.insert(name, o);
    }
    let mut state = GoldenState {
        blender: None,
        lines: line_outcomes,
        objects,
        meshes: BTreeMap::new(),
        evaluated: BTreeMap::new(),
        materials: BTreeMap::new(),
        actions: BTreeMap::new(),
        context: GContext::default(),
        files: BTreeMap::new(),
    };
    for spec in &case.compare {
        match spec {
            CompareSpec::Mesh(name) => {
                let Some(r) = d.session.main().meshes.find(name) else {
                    continue;
                };
                let m = match d.rna_mesh(&data_path("meshes", name)) {
                    Ok(m) => m,
                    Err(why) => {
                        d.fallback(format!("meshes[{name:?}]: {why}"));
                        native_mesh(d.session.main().meshes.get(r).expect("invariant: found"))
                    }
                };
                state.meshes.insert(name.clone(), m);
            }
            CompareSpec::Evaluated(name) => {
                let Some(r) = d.session.main().objects.find(name) else {
                    continue;
                };
                let base = format!(
                    "{}.evaluated_get(bpy.context.evaluated_depsgraph_get()).to_mesh()",
                    data_path("objects", name)
                );
                let m = match d.rna_mesh(&base) {
                    Ok(m) => m,
                    Err(why) => {
                        let main = d.session.main();
                        let ob = main.objects.get(r).expect("invariant: found");
                        if !ob.modifiers.is_empty() {
                            return Err(HarnessError::Frap(format!(
                                "evaluated:{name}: RNA read failed ({why}) and the object has \
                                 modifiers (native evaluation arrives with M4)"
                            )));
                        }
                        let Some(mesh) = ob
                            .data
                            .and_then(IdRef::<MeshId>::from_any)
                            .and_then(|m| main.meshes.get(m))
                        else {
                            continue;
                        };
                        let m = native_mesh(mesh);
                        d.fallback(format!("evaluated[{name:?}]: {why}"));
                        m
                    }
                };
                state.evaluated.insert(name.clone(), m);
            }
            CompareSpec::Materials => {
                let names: Vec<String> = d
                    .session
                    .main()
                    .materials
                    .iter()
                    .map(|(_, m)| m.header.name.to_string())
                    .filter(|n| !rna_values::EXCLUDED_MATERIALS.contains(&n.as_str()))
                    .collect();
                for name in names {
                    let m = match d.rna_material(&name) {
                        Ok(m) => m,
                        Err(why) => {
                            d.fallback(format!("materials[{name:?}]: {why}"));
                            native_material(d.session.main(), &name)
                                .expect("invariant: listed material")
                        }
                    };
                    state.materials.insert(name, m);
                }
            }
            CompareSpec::Anim(name) => {
                return Err(HarnessError::Frap(format!(
                    "anim:{name}: the frap action dump arrives with M6"
                )));
            }
            CompareSpec::Glb(path) => {
                let p = opts.workdir.join(path);
                if p.is_file() {
                    let canonical = gltf_canonical::canonicalize_file(&p)
                        .unwrap_or_else(|e| json!({"$error": e}));
                    state.files.insert(path.clone(), GFile::Gltf { canonical });
                }
            }
            CompareSpec::Image(path) => {
                let p = opts.workdir.join(path);
                if p.is_file() {
                    let (width, height) = image_ssim::load_png(&p)
                        .map(|i| (i.width, i.height))
                        .unwrap_or((0, 0));
                    state.files.insert(
                        path.clone(),
                        GFile::Image {
                            width,
                            height,
                            pixels: json!({"$file": p.to_string_lossy()}),
                        },
                    );
                }
            }
            CompareSpec::Objects | CompareSpec::Context => {}
        }
    }
    state.context = match d.rna_context() {
        Ok(c) => c,
        Err(why) => {
            d.fallback(format!("context: {why}"));
            d.native_context()
        }
    };
    if opts.strict_rna && !d.fallbacks.is_empty() {
        return Err(HarnessError::Frap(format!(
            "{}: RNA fallbacks with {STRICT_RNA_ENV}=1:\n  {}",
            case.id(),
            d.fallbacks.join("\n  ")
        )));
    }
    Ok(FrapRun {
        state,
        rna_fallbacks: d.fallbacks,
    })
}

fn mesh_pair<'a>(
    what: &str,
    f: Option<&'a GMesh>,
    b: Option<&'a GMesh>,
    out: &mut Vec<Mismatch>,
) -> Option<(&'a GMesh, &'a GMesh)> {
    match (f, b) {
        (Some(f), Some(b)) => Some((f, b)),
        (f, b) => {
            out.push(Mismatch::new(
                if f.is_none() && b.is_none() {
                    "harness"
                } else {
                    "rna_values"
                },
                what,
                match (f.is_some(), b.is_some()) {
                    (true, false) => "only in frap",
                    (false, true) => "only in blender",
                    _ => "missing on both sides",
                },
            ));
            None
        }
    }
}

fn compare_meshes(case: &Case, what: &str, f: &GMesh, b: &GMesh, out: &mut Vec<Mismatch>) {
    if case.tier != Tier::T3 {
        out.extend(match case.mesh_order() {
            Order::Exact => mesh_exact::compare(what, f, b, case.tol),
            Order::Canonical => mesh_canonical::compare(what, f, b, case.tol),
        });
    }
    if !case.checks.is_empty() {
        out.extend(invariants::check("frap", what, f, &case.checks));
        out.extend(invariants::check("blender", what, b, &case.checks));
    }
}

/// Compares a frap dump with a Blender dump according to the case's directives. Both states
/// must have their files resolved.
pub fn compare(case: &Case, frap: &GoldenState, blender: &GoldenState) -> Vec<Mismatch> {
    let mut out = lines::compare(
        &frap.lines,
        &blender.lines,
        &case.side_only_lines(),
        &lines::MessageNorm::default(),
    );
    for spec in &case.compare {
        match spec {
            CompareSpec::Objects => out.extend(rna_values::compare_objects(
                &frap.objects,
                &blender.objects,
                case.tol,
            )),
            CompareSpec::Context => out.extend(rna_values::compare_context(
                &frap.context,
                &blender.context,
                case.tol,
            )),
            CompareSpec::Materials => out.extend(rna_values::compare_materials(
                &frap.materials,
                &blender.materials,
                case.tol,
            )),
            CompareSpec::Mesh(n) => {
                let what = format!("meshes[{n:?}]");
                if let Some((f, b)) =
                    mesh_pair(&what, frap.meshes.get(n), blender.meshes.get(n), &mut out)
                {
                    compare_meshes(case, &what, f, b, &mut out);
                }
            }
            CompareSpec::Evaluated(n) => {
                let what = format!("evaluated[{n:?}]");
                if let Some((f, b)) = mesh_pair(
                    &what,
                    frap.evaluated.get(n),
                    blender.evaluated.get(n),
                    &mut out,
                ) {
                    compare_meshes(case, &what, f, b, &mut out);
                }
            }
            CompareSpec::Anim(n) => {
                let what = format!("actions[{n:?}]");
                let get = |s: &GoldenState| s.actions.get(n).map(|a: &GAction| a.data.clone());
                match (get(frap), get(blender)) {
                    (Some(f), Some(b)) => rna_values::json_close(
                        rna_values::NAME,
                        &what,
                        &f,
                        &b,
                        case.tol as f64,
                        &mut out,
                    ),
                    (f, _) => out.push(Mismatch::new(
                        rna_values::NAME,
                        what,
                        if f.is_some() {
                            "only in frap"
                        } else {
                            "missing in frap"
                        },
                    )),
                }
            }
            CompareSpec::Glb(p) => {
                let what = format!("files[{p:?}]");
                match (frap.files.get(p), blender.files.get(p)) {
                    (Some(GFile::Gltf { canonical: f }), Some(GFile::Gltf { canonical: b })) => {
                        out.extend(gltf_canonical::compare(&what, f, b, case.tol))
                    }
                    (f, b) => out.push(Mismatch::new(
                        gltf_canonical::NAME,
                        what,
                        format!(
                            "frap {} / blender {}",
                            if f.is_some() { "present" } else { "missing" },
                            if b.is_some() { "present" } else { "missing" }
                        ),
                    )),
                }
            }
            CompareSpec::Image(p) => {
                let what = format!("files[{p:?}]");
                let load = |f: Option<&GFile>| -> Result<frap_imbuf::DecodedImage, String> {
                    match f {
                        Some(GFile::Image { pixels, .. }) => {
                            let path = pixels
                                .get("$file")
                                .and_then(|x| x.as_str())
                                .ok_or("inline pixels are not supported")?;
                            image_ssim::load_png(Path::new(path))
                        }
                        Some(_) => Err("not an image".into()),
                        None => Err("missing".into()),
                    }
                };
                match (load(frap.files.get(p)), load(blender.files.get(p))) {
                    (Ok(f), Ok(b)) => out.extend(image_ssim::compare(
                        &what,
                        &f,
                        &b,
                        case.ssim,
                        image_ssim::DEFAULT_MAX_DELTA_E,
                    )),
                    (Err(e), _) => {
                        out.push(Mismatch::new(image_ssim::NAME, &what, format!("frap: {e}")))
                    }
                    (_, Err(e)) => out.push(Mismatch::new(
                        image_ssim::NAME,
                        &what,
                        format!("blender: {e}"),
                    )),
                }
            }
        }
    }
    out
}

/// Outcome of checking one case against its fixture.
#[derive(Clone, Debug)]
pub struct CaseReport {
    /// Case id.
    pub case: String,
    /// Comparison failures.
    pub mismatches: Vec<Mismatch>,
    /// RNA fallbacks of the frap dump.
    pub rna_fallbacks: Vec<String>,
    /// Harness failure (the case could not be compared).
    pub error: Option<String>,
    /// No fixture.
    pub unblessed: bool,
}

impl CaseReport {
    /// True when the case passed.
    pub fn passed(&self) -> bool {
        self.mismatches.is_empty() && self.error.is_none() && !self.unblessed
    }
}

/// Runs `case` through frap and compares it with its committed fixture (§20.5: `cargo test -p
/// frap-golden`). `skip_blender` cases only have to run.
pub fn check_against_fixture(
    repo: &Repo,
    props: &Props,
    case: &Case,
    strict_rna: bool,
) -> CaseReport {
    let mut rep = CaseReport {
        case: case.id(),
        mismatches: Vec::new(),
        rna_fallbacks: Vec::new(),
        error: None,
        unblessed: false,
    };
    let fixture = if case.skip_blender {
        None
    } else {
        match crate::blender::load_fixture(case, &repo.fixtures_dir()) {
            Ok(Some(f)) => Some(f),
            Ok(None) => {
                rep.unblessed = true;
                return rep;
            }
            Err(e) => {
                rep.error = Some(e.to_string());
                return rep;
            }
        }
    };
    let work = match ScratchDir::new(&case.name) {
        Ok(w) => w,
        Err(e) => {
            rep.error = Some(e.to_string());
            return rep;
        }
    };
    let run = run_frap(
        case,
        props,
        &RunOptions {
            workdir: work.path().to_owned(),
            strict_rna,
        },
    );
    match run {
        Ok(run) => {
            rep.rna_fallbacks = run.rna_fallbacks;
            if let Some(b) = fixture {
                rep.mismatches = compare(case, &run.state, &b);
            }
        }
        Err(e) => rep.error = Some(e.to_string()),
    }
    rep
}

/// Formats reports for a test failure / CLI output.
pub fn format_reports(reports: &[CaseReport], max_per_case: usize) -> String {
    let mut s = String::new();
    for r in reports.iter().filter(|r| !r.passed()) {
        s.push_str(&format!("\n{}:\n", r.case));
        if r.unblessed {
            s.push_str(&format!(
                "  not blessed: run `cargo xtask golden bless --filter {}`\n",
                r.case
            ));
        }
        if let Some(e) = &r.error {
            s.push_str(&format!("  error: {e}\n"));
        }
        for m in r.mismatches.iter().take(max_per_case) {
            s.push_str(&format!("  {m}\n"));
        }
        if r.mismatches.len() > max_per_case {
            s.push_str(&format!(
                "  … {} more mismatches\n",
                r.mismatches.len() - max_per_case
            ));
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn props_toml_parses() {
        let p = Props::load(&Repo::locate().props_path()).unwrap();
        assert_eq!(p.object.rotation["EULER"], "rotation_euler");
        assert!(p.derived.props.iter().any(|x| x == "matrix_world"));
        assert_eq!(p.mesh.attribute_value["BYTE_COLOR"], "color");
        assert!(Props::parse("version = 2").is_err());
    }

    #[test]
    fn value_parsing() {
        assert_eq!(
            ref_name(&json!({"$ref": "bpy.data.objects['It\\'s']"})).unwrap(),
            Some("It's".into())
        );
        assert_eq!(ref_name(&J::Null).unwrap(), None);
        assert!(ref_name(&json!(3)).is_err());
        assert!(f(&json!({"$float": "nan"})).unwrap().is_nan());
        assert_eq!(fixed::<3>(&json!([1, 2.5, -3])).unwrap(), [1.0, 2.5, -3.0]);
        assert!(fixed::<3>(&json!([1, 2])).is_err());
        let b = json!({"$array": [1, 2, 3, 4, 5, 6], "shape": [2, 3], "len": 2});
        assert_eq!(
            bulk_items(&b).unwrap(),
            vec![json!([1, 2, 3]), json!([4, 5, 6])]
        );
        let t = json!({"$array": [1], "shape": [1], "truncated": true});
        assert!(bulk_items(&t).is_err());
        assert_eq!(py_str("a'b\\c"), "'a\\'b\\\\c'");
        assert_eq!(
            coll_items(&json!({"$coll": "x", "len": 2, "items": ["A", "B"]})).unwrap(),
            vec!["A", "B"]
        );
    }

    #[test]
    fn native_matrices_follow_blender() {
        let mut ob = Object::new("O", None);
        ob.location = [1.0, 2.0, 3.0];
        ob.scale = [2.0, 1.0, 1.0];
        ob.rotation_euler = [0.0, 0.0, std::f32::consts::FRAC_PI_2];
        let m = local_matrix(&ob);
        // X axis scaled by 2 then rotated 90° about Z: column 0 = (0, 2, 0).
        let close = |a: f64, b: f64| (a - b).abs() < 1e-6;
        assert!(close(m[0][0], 0.0) && close(m[1][0], 2.0) && close(m[2][0], 0.0));
        assert!(close(m[0][3], 1.0) && close(m[1][3], 2.0) && close(m[2][3], 3.0));
        // Euler XYZ = Rz·Ry·Rx; the same rotation as a quaternion and as axis-angle.
        ob.rotation_euler = [0.3, 0.0, 0.0];
        let e = local_matrix(&ob);
        ob.rotation_mode = "QUATERNION".into();
        let h = 0.15f32;
        ob.rotation_quaternion = [h.cos(), h.sin(), 0.0, 0.0];
        let q = local_matrix(&ob);
        ob.rotation_mode = "AXIS_ANGLE".into();
        ob.rotation_axis_angle = [0.3, 2.0, 0.0, 0.0];
        let a = local_matrix(&ob);
        for i in 0..4 {
            for j in 0..4 {
                assert!(close(e[i][j], q[i][j]) && close(e[i][j], a[i][j]), "{i}{j}");
            }
        }
        // Order matters: ZYX differs from XYZ for a two-axis rotation.
        let xyz = euler("XYZ", [0.3, 0.4, 0.0]);
        let zyx = euler("ZYX", [0.3, 0.4, 0.0]);
        assert!(!close(xyz[0][2], zyx[0][2]) || !close(xyz[2][1], zyx[2][1]));
    }

    #[test]
    fn scrubs_the_work_directory() {
        let mut l = vec![LineOutcome {
            line: 1,
            ok: false,
            exception: Some("RuntimeError".into()),
            message: Some("cannot write /tmp/x/out/a.png".into()),
        }];
        scrub_workdir(&mut l, Path::new("/tmp/x"));
        assert_eq!(
            l[0].message.as_deref(),
            Some("cannot write <workdir>/out/a.png")
        );
    }
}
