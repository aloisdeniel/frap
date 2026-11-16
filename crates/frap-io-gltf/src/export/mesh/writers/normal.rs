// SPDX-License-Identifier: GPL-3.0-or-later
//! NORMAL writer (`export_normals`): key = the corner normal rounded to 4 decimals, normalised,
//! a zero normal replaced by +Z, then Y-up, as f32 bits with `-0.0` folded (3 words per
//! corner); `run` fills [`PrimitiveDraft::normals`] from the representative corners.
//!
//! Ported from blender/scripts/addons_core/io_scene_gltf2/blender/exp/primitive_extract.py
//! (`__get_normals`, `__get_normal_attribute`; `ROUNDING_DIGIT = 4`). Normals come from
//! `Mesh.corner_normals`, so flat, smooth, sharp-edge and custom normals all split vertices.

use crate::export::ExportError;
use crate::export::mesh::convert::{axis, key_word, normal_key};
use crate::export::mesh::ext::{
    MeshExportCx, PRIMITIVE_ATTRIBUTE_WRITERS, PrimitiveAttributeWriter, PrimitiveDraft, SplitKey,
};

/// This writer's split-key column.
pub const NAME: &str = "normal";

fn key(cx: &MeshExportCx, k: &mut SplitKey) -> Result<(), ExportError> {
    if !cx.settings.export_normals {
        return Ok(());
    }
    let normals = cx.mesh.corner_normals();
    let mut words = Vec::with_capacity(3 * normals.len());
    for n in normals.iter() {
        let n = axis(normal_key(*n), cx.settings.export_yup);
        words.extend(n.map(key_word));
    }
    k.push(NAME, 3, words);
    Ok(())
}

fn run(_: &MeshExportCx, k: &SplitKey, d: &mut PrimitiveDraft) -> Result<(), ExportError> {
    let Some(col) = k.column(NAME) else {
        return Ok(());
    };
    // Corner data only: loose edges/points (no corner) carry no NORMAL, as in the add-on.
    if d.corners.contains(&u32::MAX) {
        return Ok(());
    }
    d.normals = d
        .corners
        .iter()
        .map(|&c| {
            let w = &col.words[3 * c as usize..3 * c as usize + 3];
            [
                f32::from_bits(w[0]),
                f32::from_bits(w[1]),
                f32::from_bits(w[2]),
            ]
        })
        .collect();
    Ok(())
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(PRIMITIVE_ATTRIBUTE_WRITERS)]
static NORMAL: PrimitiveAttributeWriter = PrimitiveAttributeWriter {
    name: NAME,
    order: 100,
    key: Some(key),
    run,
};

#[cfg(test)]
mod tests {
    use frap_mesh::Mesh;
    use frap_scene::Main;

    use super::*;
    use crate::export::ExportSettings;

    fn tilted_tri() -> Mesh {
        let mut m = Mesh::new("T");
        m.from_pydata(
            &[[0.0, 0.0, 0.0], [1.0, 0.0, 0.1], [0.0, 1.0, 0.0]],
            &[],
            &[vec![0, 1, 2]],
            true,
        );
        m
    }

    #[test]
    fn rounded_normalised_yup() {
        let m = tilted_tri();
        let s = ExportSettings::default();
        let main = Main::default();
        let cx = MeshExportCx::new(&main, &m, None, &s);
        let mut k = SplitKey::new(m.corners_num());
        key(&cx, &mut k).unwrap();
        let col = k.column(NAME).unwrap();
        assert_eq!(col.stride, 3);
        let n = m.corner_normals()[0];
        let expect = axis(normal_key(n), true);
        assert_eq!(
            &col.words[..3],
            &[key_word(expect[0]), key_word(expect[1]), key_word(expect[2])]
        );
        // Y-up: Blender +Z is glTF +Y.
        assert!(f32::from_bits(col.words[1]) > 0.99);
        // Every component has at most 4 decimals before the final normalisation.
        let r = n.map(round4_check);
        assert!(r.iter().all(|x| *x));
    }

    fn round4_check(x: f32) -> bool {
        let r = crate::export::mesh::convert::round4(x);
        (r * 10000.0 - (r * 10000.0).round()).abs() < 1e-2
    }

    #[test]
    fn disabled_without_export_normals() {
        let m = tilted_tri();
        let s = ExportSettings {
            export_normals: false,
            ..ExportSettings::default()
        };
        let main = Main::default();
        let cx = MeshExportCx::new(&main, &m, None, &s);
        let mut k = SplitKey::new(m.corners_num());
        key(&cx, &mut k).unwrap();
        assert!(k.column(NAME).is_none());
        let mut d = PrimitiveDraft {
            corners: vec![0],
            ..PrimitiveDraft::default()
        };
        run(&cx, &k, &mut d).unwrap();
        assert!(d.normals.is_empty());
    }
}
