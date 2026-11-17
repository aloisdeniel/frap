// SPDX-License-Identifier: GPL-3.0-or-later
//! TEXCOORD_n writer (`export_texcoords`): every UV map in `mesh.uv_layers` order becomes
//! `TEXCOORD_<i>`, V flipped (`(u, v) → (u, 1 - v)`). Key = those values as f32 bits with
//! `-0.0` folded (2 words per map per corner); `run` fills [`PrimitiveDraft::texcoords`].
//!
//! Ported from blender/scripts/addons_core/io_scene_gltf2/blender/exp/primitive_extract.py
//! (`prepare_data`: `tex_coord_max = len(uv_layers)`; `__get_uvs_attribute`: `uvs[:, 1] *= -1;
//! uvs[:, 1] += 1`).

use crate::export::ExportError;
use crate::export::mesh::convert::key_word;
use crate::export::mesh::ext::{
    MeshExportCx, PRIMITIVE_ATTRIBUTE_WRITERS, PrimitiveAttributeWriter, PrimitiveDraft, SplitKey,
};

/// This writer's split-key column.
pub const NAME: &str = "texcoord";

/// Blender UV → glTF UV: `(u, 1 - v)` computed as numpy does (`-v + 1` in float32).
pub fn flip_v(uv: [f32; 2]) -> [f32; 2] {
    [uv[0], -uv[1] + 1.0]
}

fn key(cx: &MeshExportCx, k: &mut SplitKey) -> Result<(), ExportError> {
    if !cx.settings.export_texcoords {
        return Ok(());
    }
    let maps = cx.mesh.uv_maps();
    if maps.is_empty() {
        return Ok(());
    }
    let n = cx.mesh.corners_num();
    let mut words = Vec::with_capacity(2 * maps.len() * n);
    for c in 0..n {
        for (_, uv) in &maps {
            let [u, v] = flip_v(uv[c]);
            words.push(key_word(u));
            words.push(key_word(v));
        }
    }
    k.push(NAME, 2 * maps.len(), words);
    Ok(())
}

fn run(_: &MeshExportCx, k: &SplitKey, d: &mut PrimitiveDraft) -> Result<(), ExportError> {
    let Some(col) = k.column(NAME) else {
        return Ok(());
    };
    if d.corners.contains(&u32::MAX) {
        return Ok(());
    }
    let maps = col.stride / 2;
    d.texcoords = (0..maps)
        .map(|m| {
            d.corners
                .iter()
                .map(|&c| {
                    let at = c as usize * col.stride + 2 * m;
                    [
                        f32::from_bits(col.words[at]),
                        f32::from_bits(col.words[at + 1]),
                    ]
                })
                .collect()
        })
        .collect();
    Ok(())
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(PRIMITIVE_ATTRIBUTE_WRITERS)]
static TEXCOORD: PrimitiveAttributeWriter = PrimitiveAttributeWriter {
    name: NAME,
    order: 200,
    key: Some(key),
    run,
};

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use frap_mesh::{AttrData, Attribute, Domain, Mesh};
    use frap_scene::Main;

    use super::*;
    use crate::export::ExportSettings;

    #[test]
    fn v_is_flipped() {
        assert_eq!(flip_v([0.25, 0.0]), [0.25, 1.0]);
        assert_eq!(flip_v([0.25, 1.0]), [0.25, 0.0]);
        assert_eq!(flip_v([0.5, 0.75]), [0.5, 0.25]);
    }

    #[test]
    fn every_uv_map_in_order() {
        let mut m = Mesh::new("T");
        m.from_pydata(
            &[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            &[],
            &[vec![0, 1, 2]],
            true,
        );
        for (name, off) in [("A", 0.0f32), ("B", 0.5)] {
            m.attributes_mut().insert(Attribute {
                name: name.into(),
                domain: Domain::Corner,
                data: AttrData::Float2(Arc::new(vec![[off, 0.0], [1.0, off], [0.0, 1.0]])),
            });
        }
        let s = ExportSettings::default();
        let main = Main::default();
        let cx = MeshExportCx::new(&main, &m, None, &s);
        let mut k = SplitKey::new(3);
        key(&cx, &mut k).unwrap();
        assert_eq!(k.column(NAME).unwrap().stride, 4);
        let mut d = PrimitiveDraft {
            corners: vec![1, 0],
            ..PrimitiveDraft::default()
        };
        run(&cx, &k, &mut d).unwrap();
        assert_eq!(d.texcoords.len(), 2);
        assert_eq!(d.texcoords[0], [[1.0, 1.0], [0.0, 1.0]]);
        assert_eq!(d.texcoords[1], [[1.0, 0.5], [0.5, 1.0]]);

        let off = ExportSettings {
            export_texcoords: false,
            ..ExportSettings::default()
        };
        let cx = MeshExportCx::new(&main, &m, None, &off);
        let mut k = SplitKey::new(3);
        key(&cx, &mut k).unwrap();
        assert!(k.column(NAME).is_none());
    }
}
