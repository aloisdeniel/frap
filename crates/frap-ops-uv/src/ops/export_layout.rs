// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.uv.export_layout(filepath='', export_all=False, export_tiles='NONE',
//! modified=False, mode='PNG', size=(1024, 1024), opacity=0.25, check_existing=True)`, tier T1
//! (`tests/golden/cases/uv/edit_m8/export_layout.py`). Port of the add-on operator
//! `ExportUVLayout.execute` (blender/scripts/addons_core/io_mesh_uv_layout/__init__.py),
//! Blender 5.2.4, GPL-2.0-or-later; [`vector`] writes SVG and EPS, [`png`] draws the PNG.
//!
//! Poll: the active object is a mesh with a UV map. Leaves Edit Mode for the export and comes
//! back (as the add-on does). The faces of the active and selected meshes with an active UV
//! map (selected faces only unless `export_all`; the evaluated mesh with `modified`) are drawn
//! with their material's `diffuse_color` (0.8 grey without a material) at `opacity`, edges
//! black, into `filepath` with the `mode` extension; `export_tiles` writes one file per UV tile
//! holding UVs (`<name>.1001.png`, UDIM numbering, or `<name>.u1_v1.png`). SVG and EPS are
//! the add-on's bytes except the generator in the header; the PNG is drawn on the CPU
//! (Blender draws it on the GPU, so `blender -b` needs `gpu.init()` first). The add-on visits
//! the meshes in the order of a Python `set` of objects (by address); frap visits the active
//! one first, then the selected ones in view-layer order.

pub(crate) mod png;
pub(crate) mod py;
pub(crate) mod vector;

use std::path::Path;

use frap_mesh::{AttrData, Domain, Mesh};
use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollFail, PollReq};
use frap_rna::{IdRef, Input, ObjectId};
use frap_scene::{Context, ObjectMode};
use frap_spec_ops_uv::uv;
use frap_spec_ops_uv::uv::export_layout::{ExportTiles, Mode};

use vector::Poly;

/// `ExportUVLayout.poll`: the active object is a mesh with a UV map.
fn layout_poll(c: &Context) -> Result<(), PollFail> {
    let ok = poll::active(c).is_some_and(|(_, ob)| {
        ob.object_type() == "MESH"
            && ob
                .data
                .and_then(|d| d.typed::<frap_rna::MeshId>())
                .and_then(|me| c.main.meshes.get(me))
                .is_some_and(|me| !me.uv_maps().is_empty())
    });
    if ok {
        Ok(())
    } else {
        Err(poll::fail(
            PollReq::ActiveType("MESH"),
            "make a mesh with a UV map the active object, e.g. bpy.ops.mesh.primitive_cube_add()",
        ))
    }
}

/// The poll of `uv.export_layout`.
const LAYOUT_POLL: Poll = Poll {
    requires: &[PollReq::ActiveType("MESH")],
    check: layout_poll,
};

/// `bpy.ops.object.mode_set(mode=…, toggle=False)` as the add-on calls it.
fn mode_set(cx: &mut OpCtx, mode: &str) -> Result<(), OpError> {
    let def = frap_op::lookup("object.mode_set").map_err(|e| OpError::Runtime(e.message))?;
    let (args, _, _) = frap_op::convert_args(
        def.spec,
        Vec::new(),
        vec![("mode".into(), Input::Str(mode.into()))],
    )
    .map_err(|e| OpError::Runtime(e.message))?;
    cx.run_op("object.mode_set", args).map(|_| ())
}

/// The active UV map's corner UVs of `me` (`mesh.uv_layers.active`).
fn active_uv(me: &Mesh) -> Option<Vec<[f32; 2]>> {
    let maps = me.uv_maps();
    let pick = me
        .uv_active
        .as_deref()
        .and_then(|n| maps.iter().find(|(name, _)| *name == n))
        .or_else(|| maps.first())?;
    Some(pick.1.to_vec())
}

/// `iter_polygon_data_to_draw` for one mesh.
fn polygon_data(cx: &OpCtx, me: &Mesh, export_all: bool, out: &mut Vec<Poly>) {
    let Some(uvs) = active_uv(me) else { return };
    let offsets = me.face_offsets();
    let select = me.bool_attribute(".select_poly", Domain::Face);
    let mat_index = match me.attributes().get("material_index").map(|a| &a.data) {
        Some(AttrData::Int(v)) => Some(v.clone()),
        _ => None,
    };
    for f in 0..me.faces_num() {
        if !(export_all || select.is_some_and(|s| s.get(f).copied().unwrap_or(false))) {
            continue;
        }
        let (start, end) = (offsets[f] as usize, offsets[f + 1] as usize);
        let face_uvs = uvs[start..end]
            .iter()
            .map(|uv| [f64::from(uv[0]), f64::from(uv[1])])
            .collect();
        // `get_polygon_color`: the mesh's material at the face's index, else 0.8 grey.
        let mi = mat_index
            .as_ref()
            .and_then(|v| v.get(f).copied())
            .unwrap_or(0);
        let color = usize::try_from(mi)
            .ok()
            .and_then(|i| me.materials.get(i).copied().flatten())
            .and_then(|m| cx.main().materials.get(m))
            .map_or([0.8, 0.8, 0.8], |m| {
                [
                    f64::from(m.diffuse_color[0]),
                    f64::from(m.diffuse_color[1]),
                    f64::from(m.diffuse_color[2]),
                ]
            });
        out.push(Poly {
            uvs: face_uvs,
            color,
        });
    }
}

/// `tiles_to_export`, sorted.
fn tiles_to_export(tiles: ExportTiles, polys: &[Poly]) -> Vec<(i64, i64)> {
    if tiles == ExportTiles::None {
        return vec![(0, 0)];
    }
    let mut out: Vec<(i64, i64)> = Vec::new();
    for p in polys {
        for uv in &p.uvs {
            // UVs exactly on a tile's upper or right border do not load the next tile.
            let (u, v) = (uv[0], uv[1]);
            let (mut x, mut y) = (u.floor() as i64, v.floor() as i64);
            if x > 0 && u < x as f64 + 1e-6 {
                x -= 1;
            }
            if y > 0 && v < y as f64 + 1e-6 {
                y -= 1;
            }
            if x >= 0 && y >= 0 && !out.contains(&(x, y)) {
                out.push((x, y));
            }
        }
    }
    out.sort_unstable();
    out
}

/// `bpy.path.ensure_ext(path, ext)` (case-insensitive).
fn ensure_ext(path: &str, ext: &str) -> String {
    if path.to_lowercase().ends_with(&ext.to_lowercase()) {
        path.to_owned()
    } else {
        format!("{path}{ext}")
    }
}

/// The add-on's name regex: strips one `.png` / `.eps` / `.svg` and, before it, the UDIM
/// (`.1001`) or UVTILE (`.u1_v1`) numbering of the chosen scheme.
fn strip_name(filename: &str, tiles: ExportTiles) -> String {
    let mut name = filename;
    for ext in [".png", ".eps", ".svg"] {
        if let Some(s) = name.strip_suffix(ext) {
            name = s;
            break;
        }
    }
    let is_digits = |s: &str, n: Option<usize>| {
        !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()) && n.is_none_or(|n| s.len() == n)
    };
    match tiles {
        ExportTiles::Udim => {
            if let Some(dot) = name.rfind('.')
                && is_digits(&name[dot + 1..], Some(4))
            {
                name = &name[..dot];
            }
        }
        ExportTiles::Uv => {
            if let Some(dot) = name.rfind(".u")
                && let Some((u, v)) = name[dot + 2..].split_once("_v")
                && is_digits(u, None)
                && is_digits(v, None)
            {
                name = &name[..dot];
            }
        }
        ExportTiles::None => {}
    }
    name.to_owned()
}

/// The exception of the add-on's `open(filepath, 'w')` (Blender raises it as `RuntimeError`
/// with the Python traceback; frap keeps its last line).
fn write_error(e: &std::io::Error, path: &str) -> OpError {
    use std::io::ErrorKind as K;
    let class = match e.kind() {
        K::NotFound => "FileNotFoundError",
        K::PermissionDenied => "PermissionError",
        K::IsADirectory => "IsADirectoryError",
        K::NotADirectory => "NotADirectoryError",
        _ => "OSError",
    };
    let errno = e.raw_os_error().unwrap_or(0);
    OpError::Runtime(format!(
        "Python: {class}: [Errno {errno}] {}: '{path}'",
        frap_rna::error::strerror(e)
    ))
}

#[operator(
    poll = LAYOUT_POLL,
    tier = T1,
    notes = [
        "PNG drawn on the CPU (Blender draws it on the GPU and needs gpu.init() in background mode); SVG and EPS name frap in their generator line",
        "several meshes are visited active first, then in view-layer order (the add-on walks a Python set of objects)",
    ]
)]
fn exec(cx: &mut OpCtx, p: uv::ExportLayout) -> OpResult {
    let active = poll::active(&cx.ctx()).map(|(ob, o)| (ob, o.mode));
    let is_editmode = active.is_some_and(|(_, m)| m == ObjectMode::Edit);
    if is_editmode {
        mode_set(cx, "OBJECT")?;
    }
    // `iter_objects_to_export`: the active and the selected meshes with an active UV map.
    let mut objects: Vec<IdRef<ObjectId>> = Vec::new();
    if let Some((ob, _)) = active {
        objects.push(ob);
    }
    for ob in cx.ctx().selected_objects() {
        if !objects.contains(&ob) {
            objects.push(ob);
        }
    }
    let mut polys: Vec<Poly> = Vec::new();
    for ob in objects {
        let Some(o) = cx.main().objects.get(ob) else { continue };
        if o.object_type() != "MESH" {
            continue;
        }
        let Some(me_ref) = o.data.and_then(|d| d.typed::<frap_rna::MeshId>()) else {
            continue;
        };
        if p.modified.value {
            let eo = cx.evaluated(ob)?;
            if let Some(me) = eo.mesh.clone() {
                polygon_data(cx, &me, p.export_all.value, &mut polys);
            }
        } else if let Some(me) = cx.main().meshes.get(me_ref) {
            let me = me.clone();
            polygon_data(cx, &me, p.export_all.value, &mut polys);
        }
    }

    let tiles_mode = p.export_tiles.value;
    let tiles = tiles_to_export(tiles_mode, &polys);
    let mode = p.mode.value;
    let ext = match mode {
        Mode::Svg => ".svg",
        Mode::Eps => ".eps",
        Mode::Png => ".png",
    };
    let filepath = p.filepath.value.to_string();
    let (dirname, filename) = match filepath.rfind(['/', std::path::MAIN_SEPARATOR]) {
        Some(i) => (&filepath[..i], &filepath[i + 1..]),
        None => ("", filepath.as_str()),
    };
    let filename = strip_name(filename, tiles_mode);
    let size = p.size.value;
    let opacity = f64::from(p.opacity.value);
    let blend_name = cx
        .main()
        .filepath
        .as_deref()
        .map(|f| {
            Path::new(f)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
        })
        .unwrap_or_default();
    for tile in tiles {
        let mut path = if dirname.is_empty() {
            filename.clone()
        } else {
            format!("{dirname}/{filename}")
        };
        match tiles_mode {
            ExportTiles::Udim => path.push_str(&format!(".{:04}", 1001 + tile.0 + tile.1 * 10)),
            ExportTiles::Uv => path.push_str(&format!(".u{}_v{}", tile.0 + 1, tile.1 + 1)),
            ExportTiles::None => {}
        }
        let path = ensure_ext(&path, ext);
        let bytes = match mode {
            Mode::Svg => vector::svg(tile, &polys, size[0], size[1], opacity, &blend_name).into_bytes(),
            Mode::Eps => vector::eps(tile, &polys, size[0], size[1], opacity).into_bytes(),
            Mode::Png => {
                let rgba = png::draw(tile, &polys, size[0], size[1], opacity);
                png::encode(&rgba, size[0].max(1) as u32, size[1].max(1) as u32)
            }
        };
        let abs = cx.services().paths().resolve(&path);
        std::fs::write(&abs, bytes).map_err(|e| write_error(&e, &path))?;
    }
    if is_editmode {
        mode_set(cx, "EDIT")?;
    }
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests;
