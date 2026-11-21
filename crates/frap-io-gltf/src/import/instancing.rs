// SPDX-License-Identifier: GPL-3.0-or-later
//! `EXT_mesh_gpu_instancing` import (WP 7.5): one mesh object per instance, as the add-on's
//! `blender/imp/vnode.py::manage_gpu_instancing` creates virtual child nodes.
//!
//! Ported from io_scene_gltf2 5.2.41 `blender/imp/vnode.py` (`init_vnodes`,
//! `manage_gpu_instancing`, `get_inst_trs`) and `blender/imp/node.py` (`create_vnode`,
//! `create_object`, `create_mesh_object` for `VNode.Inst`), Apache-2.0, The glTF-Blender-IO
//! authors:
//!
//! - a node with a mesh and the extension never instantiates the mesh itself (it becomes an
//!   empty, or its camera/light): its `TRANSLATION`, `ROTATION`, `SCALE` accessors (the first
//!   one present gives the count; none, or a count of 0, gives no instance) each give one child
//!   vnode `Node_<i>.<k>` (no name, so the object is named after the mesh) of type `Inst`, with
//!   the instance TRS converted like a node's (absent attributes: identity) and the node's
//!   mesh, without skin or morph weights;
//! - instance objects are created like the node's other children (after them), are not
//!   selected after the import (`select_imported_objects` selects `VNode.Object` only), and
//!   get no extras.
//!
//! The vnodes are made by [`VTree::compute`](super::vnode::VTree) from the accessor counts
//! (`init_vnodes` has no buffers in frap); their TRS is read by [`fill_trs`] before the objects
//! are created. Custom per-instance attributes (`_*`) are ignored, as the add-on does.

use frap_gltf::Document;
use frap_gltf::ext::ext_mesh_gpu_instancing::ExtMeshGpuInstancing;

use super::convert::{quat, scale};
use super::vnode::VKind;
use super::{Gltf, ImportError};

/// Whether instancing is imported (gates the M1 WARNING of `Gltf::warn_unsupported`).
pub(crate) const IMPLEMENTED: bool = true;

/// The extension's attributes of glTF node `i`, when it has a mesh and the extension
/// (`pynode.extensions['EXT_mesh_gpu_instancing']`); a malformed extension counts as one
/// without attributes.
pub(crate) fn extension(doc: &Document, i: usize) -> Option<ExtMeshGpuInstancing> {
    let n = doc.nodes.get(i)?;
    n.mesh?;
    if !n.extensions.contains("EXT_mesh_gpu_instancing") {
        return None;
    }
    Some(
        n.extensions
            .get::<ExtMeshGpuInstancing>()
            .ok()
            .flatten()
            .unwrap_or_default(),
    )
}

/// The number of instances of `ext` (`len` of the first of `TRANSLATION`, `ROTATION`,
/// `SCALE` present); 0 when none is (`if not val: return`).
pub(crate) fn instance_count(doc: &Document, ext: &ExtMeshGpuInstancing) -> usize {
    ["TRANSLATION", "ROTATION", "SCALE"]
        .iter()
        .find_map(|k| ext.attributes.get(*k))
        .and_then(|&a| doc.accessors.get(a as usize))
        .map_or(0, |a| a.count as usize)
}

/// The elements of an instance attribute as f64 (`BinaryData.get_data_from_accessor`:
/// normalized integers divided, others raw).
fn elements(cx: &Gltf, acc: u32, n: usize) -> Result<Vec<Vec<f64>>, ImportError> {
    let a = cx.accessor(acc)?;
    let c = a.type_.components();
    if c != n {
        return Err(ImportError::Data(format!(
            "EXT_mesh_gpu_instancing: accessor {acc} has {c} components, {n} expected"
        )));
    }
    Ok(a.to_f32()
        .chunks_exact(n)
        .map(|e| e.iter().map(|&x| f64::from(x)).collect())
        .collect())
}

/// One instance attribute's elements (`None`: the attribute is absent).
type Lists = Option<Vec<Vec<f64>>>;

/// `manage_gpu_instancing`'s data: the base TRS of every instance vnode, read now that the
/// buffers are loaded (before `create_vnode` makes any object).
pub(crate) fn fill_trs(cx: &mut Gltf) -> Result<(), ImportError> {
    // Instances of one node are consecutive and in order (`VTree::compute`).
    let mut cache: Option<(usize, [Lists; 3])> = None;
    for id in 0..cx.vtree.nodes.len() {
        let v = &cx.vtree.nodes[id];
        if v.kind != VKind::Inst {
            continue;
        }
        let (Some(node), Some(k)) = (v.inst_of, v.inst_index) else {
            continue;
        };
        if cache.as_ref().is_none_or(|(n, _)| *n != node) {
            let ext = extension(&cx.doc, node).unwrap_or_default();
            let mut lists: [Lists; 3] = [None, None, None];
            for (slot, (key, n)) in [("TRANSLATION", 3), ("ROTATION", 4), ("SCALE", 3)]
                .into_iter()
                .enumerate()
            {
                if let Some(&a) = ext.attributes.get(key) {
                    lists[slot] = Some(elements(cx, a, n)?);
                }
            }
            // `assert len(trans_list) == len(rot_list) == len(scale_list)`.
            let lens: Vec<usize> = lists.iter().flatten().map(Vec::len).collect();
            if lens.windows(2).any(|w| w[0] != w[1]) {
                return Err(ImportError::Data(format!(
                    "EXT_mesh_gpu_instancing on node {node}: the instance attributes have \
                     different counts {lens:?}"
                )));
            }
            cache = Some((node, lists));
        }
        let lists = &cache.as_ref().expect("invariant: filled above").1;
        let get = |slot: usize| lists[slot].as_ref().and_then(|l| l.get(k));
        let t = get(0).map_or([0.0; 3], |e| [e[0], e[1], e[2]]);
        let r = get(1).map_or([0.0, 0.0, 0.0, 1.0], |e| [e[0], e[1], e[2], e[3]]);
        let s = get(2).map_or([1.0; 3], |e| [e[0], e[1], e[2]]);
        let base = (cx.units.loc(t), quat(r), scale(s));
        cx.vtree.nodes[id].base_trs = base;
    }
    Ok(())
}
