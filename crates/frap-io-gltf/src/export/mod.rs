// SPDX-License-Identifier: GPL-3.0-or-later
//! Scene → GLB (io_scene_gltf2 5.2.41 semantics, M0 subset: static meshes, Principled base
//! factors, one scene). Module layout follows ARCHITECTURE §17.2; M1 W0 splits it into
//! `vtree.rs`, `nodes.rs`, `mesh/*`, `materials/*`, `write.rs` and adds the extension slices.

pub mod mesh;

use std::cmp::Ordering;
use std::collections::BTreeMap;

use frap_rna::{IdRef, MaterialId, MeshId, ObjectId};
use frap_scene::id::Object;
use frap_scene::{Context, Main};
use serde_json::{Map, Value as J, json};

use crate::glb;

/// `export_format`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ExportFormat {
    /// `GLB`
    Glb,
    /// `GLTF_SEPARATE`
    GltfSeparate,
}

/// Exporter settings (M0 subset of the 110 Blender props; all others at Blender's defaults:
/// `export_apply=False`, `export_cameras=False`, `export_lights=False`, `export_yup=True`,
/// `export_normals=True`, `export_texcoords=True`, `export_materials='EXPORT'`).
#[derive(Clone, Debug)]
pub struct ExportSettings {
    /// Output format.
    pub export_format: ExportFormat,
    /// `asset.generator`.
    pub generator: String,
}

impl Default for ExportSettings {
    fn default() -> Self {
        ExportSettings {
            export_format: ExportFormat::Glb,
            generator: concat!("frap v", env!("CARGO_PKG_VERSION")).to_owned(),
        }
    }
}

/// Exporter errors.
#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    /// Feature not available in this milestone.
    #[error("{0}")]
    NotImplemented(String),
}

/// What the exporter produced.
#[derive(Clone, Debug)]
pub struct ExportOutput {
    /// File bytes.
    pub bytes: Vec<u8>,
    /// `extensionsUsed`.
    pub extensions_used: Vec<String>,
    /// Number of nodes / meshes written (reported in `Response.result`).
    pub nodes: usize,
    /// Number of glTF meshes.
    pub meshes: usize,
}

/// Natural, case-insensitive name order of the add-on's vtree roots (`Object2` < `Object10`).
fn natural_cmp(a: &str, b: &str) -> Ordering {
    fn chunks(s: &str) -> Vec<(bool, String)> {
        let mut out: Vec<(bool, String)> = Vec::new();
        for c in s.chars() {
            let d = c.is_ascii_digit();
            match out.last_mut() {
                Some((kd, buf)) if *kd == d => buf.push(c),
                _ => out.push((d, c.to_string())),
            }
        }
        out
    }
    let (ca, cb) = (chunks(&a.to_lowercase()), chunks(&b.to_lowercase()));
    for ((da, sa), (db, sb)) in ca.iter().zip(&cb) {
        let o = if *da && *db {
            let (ta, tb) = (sa.trim_start_matches('0'), sb.trim_start_matches('0'));
            ta.len().cmp(&tb.len()).then_with(|| ta.cmp(tb))
        } else {
            sa.cmp(sb)
        };
        if o != Ordering::Equal {
            return o;
        }
    }
    ca.len().cmp(&cb.len()).then_with(|| a.cmp(b))
}

/// Integral floats are written as integers (the add-on's `__fix_json`); others are the f32
/// widened to f64 and printed shortest (= Python `repr`).
fn num(x: f32) -> J {
    if x.is_finite() && x.fract() == 0.0 && x.abs() < 9.0e15 {
        J::from(x as i64)
    } else {
        J::from(x as f64)
    }
}
fn nums(v: &[f32]) -> J {
    J::Array(v.iter().map(|x| num(*x)).collect())
}

// Ported from blender/source/blender/blenlib/intern/math_rotation.cc (eul_to_quat).
fn eul_to_quat(e: [f32; 3]) -> [f32; 4] {
    let (ti, tj, th) = (e[0] * 0.5, e[1] * 0.5, e[2] * 0.5);
    let (ci, cj, ch) = (libm::cosf(ti), libm::cosf(tj), libm::cosf(th));
    let (si, sj, sh) = (libm::sinf(ti), libm::sinf(tj), libm::sinf(th));
    let (cc, cs, sc, ss) = (ci * ch, ci * sh, si * ch, si * sh);
    [
        cj * cc + sj * ss,
        cj * sc - sj * cs,
        cj * ss + sj * cc,
        cj * cs - sj * sc,
    ]
}

const SNAP: f32 = 2e-6;

struct Builder {
    bin: Vec<u8>,
    accessors: Vec<J>,
    views: Vec<J>,
    index_cache: BTreeMap<Vec<u8>, usize>,
}

impl Builder {
    fn view(&mut self, bytes: &[u8], target: u32) -> usize {
        let offset = self.bin.len();
        self.bin.extend_from_slice(bytes);
        while !self.bin.len().is_multiple_of(4) {
            self.bin.push(0);
        }
        self.views.push(
            json!({"buffer": 0, "byteLength": bytes.len(), "byteOffset": offset, "target": target}),
        );
        self.views.len() - 1
    }
    fn vec_accessor<const N: usize>(&mut self, data: &[[f32; N]], minmax: bool) -> usize {
        let bytes: Vec<u8> = data
            .iter()
            .flat_map(|v| v.iter().flat_map(|c| c.to_le_bytes()))
            .collect();
        let view = self.view(&bytes, 34962);
        let ty = match N {
            2 => "VEC2",
            3 => "VEC3",
            _ => "VEC4",
        };
        let mut a = Map::new();
        a.insert("bufferView".into(), J::from(view));
        a.insert("componentType".into(), J::from(5126));
        a.insert("count".into(), J::from(data.len()));
        if minmax {
            let mut mn = [f32::INFINITY; N];
            let mut mx = [f32::NEG_INFINITY; N];
            for v in data {
                for i in 0..N {
                    mn[i] = mn[i].min(v[i]);
                    mx[i] = mx[i].max(v[i]);
                }
            }
            a.insert("max".into(), nums(&mx));
            a.insert("min".into(), nums(&mn));
        }
        a.insert("type".into(), J::from(ty));
        self.accessors.push(J::Object(a));
        self.accessors.len() - 1
    }
    /// Index accessors with identical bytes are shared, as the add-on's gather cache does.
    fn index_accessor(&mut self, indices: &[u32]) -> usize {
        let max = indices.iter().copied().max().unwrap_or(0);
        let (bytes, ctype): (Vec<u8>, u32) = if max < 65535 {
            (
                indices
                    .iter()
                    .flat_map(|i| (*i as u16).to_le_bytes())
                    .collect(),
                5123,
            )
        } else {
            (indices.iter().flat_map(|i| i.to_le_bytes()).collect(), 5125)
        };
        let mut key = bytes.clone();
        key.extend_from_slice(&ctype.to_le_bytes());
        if let Some(i) = self.index_cache.get(&key) {
            return *i;
        }
        let view = self.view(&bytes, 34963);
        self.accessors
            .push(json!({"bufferView": view, "componentType": ctype, "count": indices.len(), "type": "SCALAR"}));
        let i = self.accessors.len() - 1;
        self.index_cache.insert(key, i);
        i
    }
}

/// Principled BSDF base factors → `pbrMetallicRoughness` (defaults omitted, as the add-on does).
/// M0 stand-in that reads the unlinked input values of the "Principled BSDF" node; WP 1.16
/// replaces it with `frap_material::lower` (§16.2).
fn material_json(mat: &frap_scene::id::Material) -> J {
    use frap_nodes::SocketValue;
    let input = |name: &str| {
        mat.node_tree
            .nodes
            .iter()
            .find(|n| n.bl_idname == "ShaderNodeBsdfPrincipled")
            .and_then(|n| n.inputs.iter().find(|s| s.enabled && s.name == name))
            .map(|s| &s.default_value)
    };
    let base_color = match input("Base Color") {
        Some(SocketValue::Color(c)) => *c,
        _ => [1.0; 4],
    };
    let factor = |name: &str| match input(name) {
        Some(SocketValue::Float(f)) => *f,
        _ => 1.0,
    };
    let (metallic, roughness) = (factor("Metallic"), factor("Roughness"));
    let mut pbr = Map::new();
    if base_color != [1.0; 4] {
        pbr.insert("baseColorFactor".into(), nums(&base_color));
    }
    if metallic != 1.0 {
        pbr.insert("metallicFactor".into(), num(metallic));
    }
    if roughness != 1.0 {
        pbr.insert("roughnessFactor".into(), num(roughness));
    }
    let mut m = Map::new();
    if !mat.use_backface_culling {
        m.insert("doubleSided".into(), J::from(true));
    }
    m.insert("name".into(), J::from(mat.header.name.as_str()));
    if !pbr.is_empty() {
        m.insert("pbrMetallicRoughness".into(), J::Object(pbr));
    }
    J::Object(m)
}

fn scene_objects(main: &Main, ctx: &Context) -> Vec<IdRef<ObjectId>> {
    let scene = ctx.scene();
    let mut out: Vec<IdRef<ObjectId>> = scene.collection.objects.clone();
    let mut stack: Vec<_> = scene.collection.children.iter().rev().copied().collect();
    while let Some(c) = stack.pop() {
        if let Some(coll) = main.collections.get(c) {
            for o in &coll.objects {
                if !out.contains(o) {
                    out.push(*o);
                }
            }
            stack.extend(coll.children.iter().rev().copied());
        }
    }
    out
}

fn node_json(name: &str, ob: &Object, mesh: Option<usize>) -> J {
    let mut n = Map::new();
    if let Some(m) = mesh {
        n.insert("mesh".into(), J::from(m));
    }
    n.insert("name".into(), J::from(name));
    let q = eul_to_quat(ob.rotation_euler); // (w, x, y, z), Blender space
    let qy = [q[1], q[3], -q[2], q[0]]; // glTF xyzw of (w, x, z, -y)
    if (qy[3] - 1.0).abs() > SNAP || qy[..3].iter().any(|c| c.abs() > SNAP) {
        n.insert("rotation".into(), nums(&qy));
    }
    let s = [ob.scale[0], ob.scale[2], ob.scale[1]];
    if s.iter().any(|c| (c - 1.0).abs() > SNAP) {
        n.insert("scale".into(), nums(&s));
    }
    let t = mesh::primitives::yup(ob.location);
    if t.iter().any(|c| c.abs() > SNAP) {
        n.insert("translation".into(), nums(&t));
    }
    J::Object(n)
}

/// Exports the context scene.
pub fn export(
    main: &Main,
    ctx: &Context,
    settings: &ExportSettings,
) -> Result<ExportOutput, ExportError> {
    if settings.export_format != ExportFormat::Glb {
        return Err(ExportError::NotImplemented(
            "frap M0: export_format='GLTF_SEPARATE' arrives in M3; use 'GLB'".into(),
        ));
    }
    // vtree roots: parentless objects, natural case-insensitive order; cameras/lights are
    // filtered out (export_cameras/export_lights default False); M0 has no parenting.
    let mut roots: Vec<(IdRef<ObjectId>, &Object)> = scene_objects(main, ctx)
        .into_iter()
        .filter_map(|r| main.objects.get(r).map(|o| (r, o)))
        .filter(|(_, o)| matches!(o.object_type(), "MESH" | "EMPTY"))
        .collect();
    roots.sort_by(|a, b| natural_cmp(&a.1.header.name, &b.1.header.name));

    let mut b = Builder {
        bin: Vec::new(),
        accessors: Vec::new(),
        views: Vec::new(),
        index_cache: BTreeMap::new(),
    };
    let mut nodes = Vec::new();
    let mut meshes: Vec<J> = Vec::new();
    let mut mesh_index: BTreeMap<IdRef<MeshId>, usize> = BTreeMap::new();
    let mut materials: Vec<J> = Vec::new();
    let mut material_index: BTreeMap<IdRef<MaterialId>, usize> = BTreeMap::new();

    for (_, ob) in &roots {
        let mut mesh_slot = None;
        if let Some(me_ref) = ob.data.and_then(|d| d.typed::<MeshId>()) {
            if let Some(i) = mesh_index.get(&me_ref) {
                mesh_slot = Some(*i);
            } else if let Some(me) = main.meshes.get(me_ref) {
                let mut prims = Vec::new();
                for p in mesh::primitives::extract(me) {
                    let mut attrs = Map::new();
                    attrs.insert(
                        "POSITION".into(),
                        J::from(b.vec_accessor(&p.positions, true)),
                    );
                    attrs.insert("NORMAL".into(), J::from(b.vec_accessor(&p.normals, false)));
                    for (k, tc) in p.texcoords.iter().enumerate() {
                        attrs.insert(format!("TEXCOORD_{k}"), J::from(b.vec_accessor(tc, false)));
                    }
                    let mut prim = Map::new();
                    prim.insert("attributes".into(), J::Object(attrs));
                    prim.insert("indices".into(), J::from(b.index_accessor(&p.indices)));
                    if let Some(Some(mat_ref)) = me.materials.get(p.material_slot)
                        && let Some(mat) = main.materials.get(*mat_ref)
                    {
                        let idx = *material_index.entry(*mat_ref).or_insert_with(|| {
                            materials.push(material_json(mat));
                            materials.len() - 1
                        });
                        prim.insert("material".into(), J::from(idx));
                    }
                    prims.push(J::Object(prim));
                }
                if !prims.is_empty() {
                    meshes.push(json!({"name": me.header.name.as_str(), "primitives": prims}));
                    mesh_index.insert(me_ref, meshes.len() - 1);
                    mesh_slot = Some(meshes.len() - 1);
                }
            }
        }
        nodes.push(node_json(&ob.header.name, ob, mesh_slot));
    }

    // Fixed top-level key order of the add-on's writer; empty arrays are dropped.
    let mut root = Map::new();
    root.insert(
        "asset".into(),
        json!({"generator": settings.generator, "version": "2.0"}),
    );
    root.insert("scene".into(), J::from(0));
    let scene_name = ctx.scene().header.name.as_str();
    let mut sc = Map::new();
    sc.insert("name".into(), J::from(scene_name));
    if !nodes.is_empty() {
        sc.insert(
            "nodes".into(),
            J::Array((0..nodes.len()).map(J::from).collect()),
        );
    }
    root.insert("scenes".into(), J::Array(vec![J::Object(sc)]));
    let n_nodes = nodes.len();
    let n_meshes = meshes.len();
    for (k, v) in [
        ("nodes", nodes),
        ("materials", materials),
        ("meshes", meshes),
    ] {
        if !v.is_empty() {
            root.insert(k.into(), J::Array(v));
        }
    }
    if !b.accessors.is_empty() {
        root.insert(
            "accessors".into(),
            J::Array(std::mem::take(&mut b.accessors)),
        );
        root.insert("bufferViews".into(), J::Array(std::mem::take(&mut b.views)));
        root.insert("buffers".into(), json!([{"byteLength": b.bin.len()}]));
    }
    let json_bytes = serde_json::to_vec(&J::Object(root)).expect("invariant: JSON serializes");
    Ok(ExportOutput {
        bytes: glb::write(&json_bytes, &b.bin),
        extensions_used: vec![],
        nodes: n_nodes,
        meshes: n_meshes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn natural_order() {
        let mut v = vec!["Object10", "object2", "Cube.001", "cube", "Cube"];
        v.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(v, ["Cube", "cube", "Cube.001", "object2", "Object10"]);
    }

    #[test]
    fn numbers_like_the_addon() {
        assert_eq!(num(1.0).to_string(), "1");
        assert_eq!(num(-0.0).to_string(), "0");
        assert_eq!(num(0.8).to_string(), "0.800000011920929");
        assert_eq!(eul_to_quat([0.0; 3]), [1.0, 0.0, 0.0, 0.0]);
    }
}
