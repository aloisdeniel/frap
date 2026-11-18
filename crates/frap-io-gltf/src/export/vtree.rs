// SPDX-License-Identifier: GPL-3.0-or-later
//! The add-on's virtual export tree (ARCHITECTURE §17.2): which objects become nodes, their
//! hierarchy after filtering, and their world matrices.
//!
//! Ported from blender/scripts/addons_core/io_scene_gltf2/blender/exp/tree.py (`VExportTree`:
//! `construct`, `recursive_node_traverse`, `filter`, `recursive_filter_tag`,
//! `recursive_filter`, `node_filter_*_is_kept`, `break_obj_hierarchy`):
//!
//! - Roots are the scene's objects without a parent, in **natural, case-insensitive** name order
//!   (`_natural_sort_key`: `Object2` < `Object10`), ties in scene order (Python's sort is
//!   stable).
//! - Children are every object of `bpy.data.objects` whose parent is the node, even objects that
//!   are not in the exported scene (the add-on builds `blender_children` from `bpy.data.objects`),
//!   in the same natural order. Objects parented to bones are left to M6 (skipped here).
//! - Filters (`use_selection`, `use_visible`, `use_renderable`, `collection`,
//!   `use_active_collection[_with_nested]`, the `glTF_not_exported` collection, cameras and
//!   lights) drop a node; its kept descendants move to the nearest kept ancestor (appended to
//!   its children) or become roots (appended to the roots).
//! - `matrix_world` is Blender's `parent.matrix_world @ matrix_parent_inverse @ matrix_basis`.
//!
//! M1 limits: cameras and lights are always filtered out (`export_cameras`/`export_lights`
//! arrive with M3 WP 3.4); collection hierarchies (`export_hierarchy_full_collections`),
//! instancing and bones are later milestones.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use frap_math::glam::Mat4;
use frap_math::matrix::{ObjectTransform, RotationMode, RotationValues};
use frap_rna::{CollectionId, IdRef, ObjectId};
use frap_scene::id::{Collection, Object};
use frap_scene::{BaseFlag, Context, LayerCollection, Main};

use crate::export::ExportSettings;

/// Name of the add-on's special collection whose objects are never exported
/// (`BLENDER_GLTF_SPECIAL_COLLECTION`).
pub const SPECIAL_COLLECTION: &str = "glTF_not_exported";

/// One node of the export tree.
#[derive(Clone, Debug)]
pub struct VNode {
    /// The object.
    pub object: IdRef<ObjectId>,
    /// Parent node after filtering.
    pub parent: Option<usize>,
    /// Children after filtering, in export order.
    pub children: Vec<usize>,
    /// `matrix_world` (Blender space).
    pub matrix_world: Mat4,
    /// Survived the filters.
    pub keep: bool,
}

/// The export tree of one scene.
#[derive(Clone, Debug, Default)]
pub struct VTree {
    /// Every node, in construction (pre-)order; dropped nodes stay with `keep == false`.
    pub nodes: Vec<VNode>,
    /// Root nodes in export order.
    pub roots: Vec<usize>,
}

/// `_natural_sort_key` order: names split into text and digit runs (`re.split(r'(\d+)')`),
/// text compared lower-cased, digit runs compared as integers.
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    fn key(s: &str) -> Vec<(bool, String)> {
        // Alternating text / number chunks, starting and ending with (possibly empty) text.
        let mut out = vec![(false, String::new())];
        for c in s.chars() {
            let digit = c.is_ascii_digit();
            let last = out.last_mut().expect("never empty");
            if last.0 == digit {
                last.1.push(c);
            } else {
                out.push((digit, c.to_string()));
            }
        }
        if out.last().is_some_and(|l| l.0) {
            out.push((false, String::new()));
        }
        out.into_iter()
            .map(|(d, t)| {
                if d {
                    (d, t.trim_start_matches('0').to_owned())
                } else {
                    (d, t.to_lowercase())
                }
            })
            .collect()
    }
    let (ka, kb) = (key(a), key(b));
    for ((da, ta), (_, tb)) in ka.iter().zip(&kb) {
        let o = if *da {
            ta.len().cmp(&tb.len()).then_with(|| ta.cmp(tb))
        } else {
            ta.cmp(tb)
        };
        if o != Ordering::Equal {
            return o;
        }
    }
    ka.len().cmp(&kb.len())
}

/// `Object.matrix_basis` (`BKE_object_to_mat4`).
pub fn matrix_basis(ob: &Object) -> Mat4 {
    let t = ObjectTransform {
        location: ob.location,
        rotation_mode: RotationMode::from_blender(&ob.rotation_mode).unwrap_or_default(),
        rotation: RotationValues {
            quaternion: ob.rotation_quaternion,
            euler: ob.rotation_euler,
            axis_angle: ob.rotation_axis_angle,
        },
        scale: ob.scale,
        delta_location: ob.delta_location,
        delta_rotation_euler: ob.delta_rotation_euler,
        delta_rotation_quaternion: ob.delta_rotation_quaternion,
        delta_scale: ob.delta_scale,
    };
    t.to_mat4()
}

/// Blender's `float[4][4]` (`m[col][row]`, translation in `m[3]`).
type M4 = [[f32; 4]; 4];

fn m4(m: &Mat4) -> M4 {
    m.to_cols_array_2d()
}

fn from_m4(m: &M4) -> Mat4 {
    Mat4::from_cols_array_2d(m)
}

// Ported from blender/source/blender/blenlib/intern/math_matrix.cc (mul_m4_m4m4: each
// component summed left to right in float).
/// `mul_m4_m4m4(a, b)` = `a @ b` as Blender's C code computes object matrices.
pub fn mul_m4(a: &Mat4, b: &Mat4) -> Mat4 {
    let (a, b) = (m4(a), m4(b));
    let mut r = [[0.0f32; 4]; 4];
    for (j, col) in r.iter_mut().enumerate() {
        for (i, v) in col.iter_mut().enumerate() {
            *v = a[0][i] * b[j][0] + a[1][i] * b[j][1] + a[2][i] * b[j][2] + a[3][i] * b[j][3];
        }
    }
    from_m4(&r)
}

// Ported from blender/source/blender/python/mathutils/mathutils_Matrix.cc (Matrix_matmul:
// float products accumulated in double, then rounded to float).
/// `a @ b` as Python's `mathutils.Matrix` computes it (the add-on's matrix products).
pub fn matmul(a: &Mat4, b: &Mat4) -> Mat4 {
    let (a, b) = (m4(a), m4(b));
    let mut r = [[0.0f32; 4]; 4];
    for (j, col) in r.iter_mut().enumerate() {
        for (i, v) in col.iter_mut().enumerate() {
            let mut dot = 0.0f64;
            for (k, ak) in a.iter().enumerate() {
                dot += f64::from(ak[i] * b[j][k]);
            }
            *v = dot as f32;
        }
    }
    from_m4(&r)
}

/// `Object.matrix_world` of the original object: `parent.matrix_world @ matrix_parent_inverse
/// @ matrix_basis`, associated as `BKE_object_where_is_calc` does (`mul_m4_series`); M1: every
/// parent type is treated as `OBJECT`.
pub fn matrix_world(main: &Main, ob: &Object) -> Mat4 {
    fn rec(main: &Main, ob: &Object, depth: usize) -> Mat4 {
        let basis = matrix_basis(ob);
        // Bounded: a parent cycle (impossible in Blender) cannot recurse forever.
        match ob.parent.and_then(|p| main.objects.get(p)) {
            Some(p) if depth < main.objects.len() => {
                let pw = rec(main, p, depth + 1);
                mul_m4(&mul_m4(&pw, &ob.matrix_parent_inverse), &basis)
            }
            _ => basis,
        }
    }
    rec(main, ob, 0)
}

// Ported from blender/source/blender/blenlib/intern/math_matrix.cc (determinant_m3,
// determinant_m4, adjoint_m4_m4) and mathutils_Matrix.cc (matrix_invert_safe_internal,
// matrix_invert_with_det_n_internal).
#[allow(clippy::too_many_arguments)]
fn det3(a1: f32, a2: f32, a3: f32, b1: f32, b2: f32, b3: f32, c1: f32, c2: f32, c3: f32) -> f32 {
    a1 * (b2 * c3 - b3 * c2) - a2 * (b1 * c3 - b3 * c1) + a3 * (b1 * c2 - b2 * c1)
}

fn det4(m: &M4) -> f32 {
    let [
        [a1, b1, c1, d1],
        [a2, b2, c2, d2],
        [a3, b3, c3, d3],
        [a4, b4, c4, d4],
    ] = *m;
    a1 * det3(b2, b3, b4, c2, c3, c4, d2, d3, d4) - b1 * det3(a2, a3, a4, c2, c3, c4, d2, d3, d4)
        + c1 * det3(a2, a3, a4, b2, b3, b4, d2, d3, d4)
        - d1 * det3(a2, a3, a4, b2, b3, b4, c2, c3, c4)
}

fn adjoint4(m: &M4) -> M4 {
    let [
        [a1, b1, c1, d1],
        [a2, b2, c2, d2],
        [a3, b3, c3, d3],
        [a4, b4, c4, d4],
    ] = *m;
    let mut r = [[0.0f32; 4]; 4];
    r[0][0] = det3(b2, b3, b4, c2, c3, c4, d2, d3, d4);
    r[1][0] = -det3(a2, a3, a4, c2, c3, c4, d2, d3, d4);
    r[2][0] = det3(a2, a3, a4, b2, b3, b4, d2, d3, d4);
    r[3][0] = -det3(a2, a3, a4, b2, b3, b4, c2, c3, c4);
    r[0][1] = -det3(b1, b3, b4, c1, c3, c4, d1, d3, d4);
    r[1][1] = det3(a1, a3, a4, c1, c3, c4, d1, d3, d4);
    r[2][1] = -det3(a1, a3, a4, b1, b3, b4, d1, d3, d4);
    r[3][1] = det3(a1, a3, a4, b1, b3, b4, c1, c3, c4);
    r[0][2] = det3(b1, b2, b4, c1, c2, c4, d1, d2, d4);
    r[1][2] = -det3(a1, a2, a4, c1, c2, c4, d1, d2, d4);
    r[2][2] = det3(a1, a2, a4, b1, b2, b4, d1, d2, d4);
    r[3][2] = -det3(a1, a2, a4, b1, b2, b4, c1, c2, c4);
    r[0][3] = -det3(b1, b2, b3, c1, c2, c3, d1, d2, d3);
    r[1][3] = det3(a1, a2, a3, c1, c2, c3, d1, d2, d3);
    r[2][3] = -det3(a1, a2, a3, b1, b2, b3, d1, d2, d3);
    r[3][3] = det3(a1, a2, a3, b1, b2, b3, c1, c2, c3);
    r
}

/// `Matrix.inverted_safe()`: the adjoint divided by the determinant; a singular matrix gets
/// `PSEUDOINVERSE_EPSILON` (1e-8) added to its diagonal first (and the determinant becomes that
/// epsilon if it is still zero).
pub fn inverted_safe(m: &Mat4) -> Mat4 {
    const EPS: f32 = 1e-8;
    let mut src = m4(m);
    let mut det = det4(&src);
    if det == 0.0 {
        for (i, col) in src.iter_mut().enumerate() {
            col[i] += EPS;
        }
        det = det4(&src);
        if det == 0.0 {
            det = EPS;
        }
    }
    let adj = adjoint4(&src);
    let mut r = [[0.0f32; 4]; 4];
    for (c, col) in r.iter_mut().enumerate() {
        for (k, v) in col.iter_mut().enumerate() {
            *v = adj[c][k] / det;
        }
    }
    from_m4(&r)
}

/// Every object of the scene (`scene.objects`): the master collection's objects, then each
/// child collection depth-first, without duplicates.
pub fn scene_objects(main: &Main, ctx: &Context) -> Vec<IdRef<ObjectId>> {
    let scene = ctx.scene();
    let mut out: Vec<IdRef<ObjectId>> = Vec::new();
    let mut seen = BTreeSet::new();
    let mut push = |o: IdRef<ObjectId>, out: &mut Vec<_>| {
        if seen.insert(o) {
            out.push(o);
        }
    };
    for o in &scene.collection.objects {
        push(*o, &mut out);
    }
    let mut stack: Vec<_> = scene.collection.children.iter().rev().copied().collect();
    let mut visited = BTreeSet::new();
    while let Some(c) = stack.pop() {
        if !visited.insert(c) {
            continue;
        }
        if let Some(coll) = main.collections.get(c) {
            for o in &coll.objects {
                push(*o, &mut out);
            }
            stack.extend(coll.children.iter().rev().copied());
        }
    }
    out
}

/// `Collection.all_objects`: the collection's objects and its children's, recursively.
fn all_objects(main: &Main, coll: &Collection) -> BTreeSet<IdRef<ObjectId>> {
    let mut out: BTreeSet<_> = coll.objects.iter().copied().collect();
    let mut stack: Vec<IdRef<CollectionId>> = coll.children.clone();
    let mut visited = BTreeSet::new();
    while let Some(c) = stack.pop() {
        if !visited.insert(c) {
            continue;
        }
        if let Some(cc) = main.collections.get(c) {
            out.extend(cc.objects.iter().copied());
            stack.extend(cc.children.iter().copied());
        }
    }
    out
}

/// `Object.users_collection`: every collection (scene master collections included) that links
/// the object directly.
fn users_collection(main: &Main, ob: IdRef<ObjectId>) -> Vec<&Collection> {
    let mut out: Vec<&Collection> = main
        .collections
        .iter()
        .map(|(_, c)| c)
        .filter(|c| c.objects.contains(&ob))
        .collect();
    out.extend(
        main.scenes
            .iter()
            .map(|(_, s)| &s.collection)
            .filter(|c| c.objects.contains(&ob)),
    );
    out
}

/// `Object.visible_get()` in the context view layer: a base that is not hidden, the object not
/// `hide_viewport`, and linked from a collection reached through layer collections that are
/// neither excluded nor hidden (`LayerCollection.hide_viewport`, `Collection.hide_viewport`).
pub fn visible_get(main: &Main, ctx: &Context, r: IdRef<ObjectId>, ob: &Object) -> bool {
    let vl = ctx.view_layer();
    let Some(base) = vl.bases.iter().find(|b| b.object == r) else {
        return false;
    };
    if base.flag.contains(BaseFlag::HIDDEN) || ob.hide_viewport {
        return false;
    }
    fn walk(main: &Main, ctx: &Context, lc: &LayerCollection, r: IdRef<ObjectId>) -> bool {
        if lc.exclude || lc.hide_viewport {
            return false;
        }
        let coll = match lc.collection {
            None => &ctx.scene().collection,
            Some(c) => match main.collections.get(c) {
                Some(c) => c,
                None => return false,
            },
        };
        if coll.hide_viewport {
            return false;
        }
        coll.objects.contains(&r) || lc.children.iter().any(|c| walk(main, ctx, c, r))
    }
    walk(main, ctx, &vl.layer_collection, r)
}

/// `Object.select_get()` in the context view layer.
pub fn select_get(ctx: &Context, r: IdRef<ObjectId>) -> bool {
    ctx.view_layer()
        .bases
        .iter()
        .any(|b| b.object == r && b.selected())
}

struct Filters<'a> {
    main: &'a Main,
    ctx: &'a Context<'a>,
    s: &'a ExportSettings,
    /// `collection` export: the collection's `all_objects` (`None` inside = not found).
    collection: Option<Option<BTreeSet<IdRef<ObjectId>>>>,
    /// Active-collection export: its `objects` or `all_objects`.
    active: Option<BTreeSet<IdRef<ObjectId>>>,
    /// Objects of `glTF_not_exported`.
    special: BTreeSet<IdRef<ObjectId>>,
}

impl<'a> Filters<'a> {
    fn new(main: &'a Main, ctx: &'a Context<'a>, s: &'a ExportSettings) -> Self {
        let collection = (!s.collection.is_empty()).then(|| {
            main.collections
                .find_in(&s.collection, None)
                .and_then(|c| main.collections.get(c))
                .map(|c| all_objects(main, c))
        });
        let active = (s.collection.is_empty() && s.use_active_collection).then(|| {
            let coll = match ctx.collection() {
                None => Some(&ctx.scene().collection),
                Some(c) => main.collections.get(c),
            };
            match coll {
                None => BTreeSet::new(),
                Some(c) if s.use_active_collection_with_nested => all_objects(main, c),
                Some(c) => c.objects.iter().copied().collect(),
            }
        });
        let special = main
            .collections
            .find(SPECIAL_COLLECTION)
            .and_then(|c| main.collections.get(c))
            .map(|c| c.objects.iter().copied().collect())
            .unwrap_or_default();
        Filters {
            main,
            ctx,
            s,
            collection,
            active,
            special,
        }
    }

    /// `node_filter_not_inheritable_is_kept`.
    fn not_inheritable(&self, ob: &Object) -> bool {
        // export_cameras / export_lights (WP 3.4, `cameras.rs` / `lights.rs`; both default to
        // False).
        match ob.object_type() {
            "CAMERA" => crate::export::cameras::exported(self.s),
            "LIGHT" => crate::export::lights::exported(self.s),
            _ => true,
        }
    }

    /// `node_filter_inheritable_is_kept`.
    fn inheritable(&self, r: IdRef<ObjectId>, ob: &Object) -> bool {
        if self.s.use_selection && !select_get(self.ctx, r) {
            return false;
        }
        if self.s.use_visible {
            if !visible_get(self.main, self.ctx, r, ob) || ob.hide_viewport {
                return false;
            }
            let users = users_collection(self.main, r);
            if users.iter().all(|c| c.hide_viewport) {
                return false;
            }
        }
        if self.s.use_renderable {
            if ob.hide_render {
                return false;
            }
            let users = users_collection(self.main, r);
            if users.iter().all(|c| c.hide_render) {
                return false;
            }
        }
        match (&self.collection, &self.active) {
            (Some(None), _) => return false,
            (Some(Some(set)), _) if !set.contains(&r) => return false,
            (None, Some(set)) if !set.contains(&r) => return false,
            _ => {}
        }
        !self.special.contains(&r)
    }
}

/// Builds the export tree of the context scene.
pub fn build(main: &Main, ctx: &Context, s: &ExportSettings) -> VTree {
    let mut children_of: BTreeMap<IdRef<ObjectId>, Vec<(IdRef<ObjectId>, &Object)>> =
        BTreeMap::new();
    for (r, ob) in main.objects.iter() {
        if let Some(p) = ob.parent
            && main.objects.get(p).is_some()
        {
            children_of.entry(p).or_default().push((r, ob));
        }
    }
    let mut roots: Vec<(IdRef<ObjectId>, &Object)> = scene_objects(main, ctx)
        .into_iter()
        .filter_map(|r| main.objects.get(r).map(|o| (r, o)))
        .filter(|(_, o)| o.parent.and_then(|p| main.objects.get(p)).is_none())
        .collect();
    roots.sort_by(|a, b| natural_cmp(&a.1.header.name, &b.1.header.name));

    let mut tree = VTree::default();
    let mut visited = BTreeSet::new();
    let filters = Filters::new(main, ctx, s);
    // Construction + keep tags (`recursive_node_traverse`, `recursive_filter_tag`).
    let mut stack: Vec<(IdRef<ObjectId>, &Object, Option<usize>)> =
        roots.iter().rev().map(|(r, o)| (*r, *o, None)).collect();
    while let Some((r, ob, parent)) = stack.pop() {
        if !visited.insert(r) {
            continue;
        }
        let i = tree.nodes.len();
        tree.nodes.push(VNode {
            object: r,
            parent,
            children: Vec::new(),
            matrix_world: matrix_world(main, ob),
            keep: filters.not_inheritable(ob) && filters.inheritable(r, ob),
        });
        match parent {
            Some(p) => tree.nodes[p].children.push(i),
            None => tree.roots.push(i),
        }
        let mut kids: Vec<(IdRef<ObjectId>, &Object)> = children_of
            .get(&r)
            .map(|v| {
                v.iter()
                    .filter(|(_, c)| {
                        // Objects parented to bones are gathered with their armature (M6).
                        !(matches!(c.parent_type.as_str(), "BONE" | "BONE_RELATIVE")
                            && !c.parent_bone.is_empty())
                    })
                    .copied()
                    .collect()
            })
            .unwrap_or_default();
        kids.sort_by(|a, b| natural_cmp(&a.1.header.name, &b.1.header.name));
        // Pre-order: push in reverse so the first child is visited next.
        for (cr, co) in kids.into_iter().rev() {
            stack.push((cr, co, Some(i)));
        }
    }
    filter(&mut tree);
    if s.export_hierarchy_flatten_objs {
        flatten(&mut tree);
    }
    tree
}

/// `recursive_filter`: drops nodes with `keep == false`, moving kept descendants up.
fn filter(tree: &mut VTree) {
    fn rec(tree: &mut VTree, i: usize, parent_kept: Option<usize>) {
        let children = tree.nodes[i].children.clone();
        let new_parent_kept;
        if !tree.nodes[i].keep {
            new_parent_kept = parent_kept;
            match tree.nodes[i].parent {
                Some(p) => tree.nodes[p].children.retain(|c| *c != i),
                None => tree.roots.retain(|c| *c != i),
            }
        } else {
            new_parent_kept = Some(i);
            let parent = tree.nodes[i].parent;
            if let Some(pk) = parent_kept
                && parent != Some(pk)
            {
                tree.nodes[pk].children.push(i);
            }
            if parent.is_some() && parent_kept.is_none() {
                tree.roots.push(i);
            }
            tree.nodes[i].parent = parent_kept;
        }
        for c in children {
            rec(tree, c, new_parent_kept);
        }
    }
    for r in tree.roots.clone() {
        rec(tree, r, None);
    }
}

/// `break_obj_hierarchy` (`export_hierarchy_flatten_objs`): every kept node becomes a root, in
/// construction order.
fn flatten(tree: &mut VTree) {
    for i in 0..tree.nodes.len() {
        if !tree.nodes[i].keep {
            continue;
        }
        if let Some(p) = tree.nodes[i].parent.take() {
            tree.nodes[p].children.retain(|c| *c != i);
            tree.roots.push(i);
        }
    }
}

impl VTree {
    /// The node's matrix relative to its (kept) parent, as `__gather_trans_rot_scale` computes
    /// it: `parent.matrix_world.inverted_safe() @ matrix_world`, or the world matrix of a root.
    pub fn local_matrix(&self, i: usize) -> Mat4 {
        let n = &self.nodes[i];
        match n.parent {
            Some(p) => matmul(&inverted_safe(&self.nodes[p].matrix_world), &n.matrix_world),
            None => n.matrix_world,
        }
    }

    /// Kept nodes in pre-order (roots in export order, then their subtrees).
    pub fn kept(&self) -> Vec<usize> {
        let mut out = Vec::new();
        let mut stack: Vec<usize> = self.roots.iter().rev().copied().collect();
        while let Some(i) = stack.pop() {
            out.push(i);
            stack.extend(self.nodes[i].children.iter().rev().copied());
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn natural_order() {
        let mut v = vec![
            "Object10", "object2", "Cube.001", "cube", "Cube", "Cube.010", "Cube.9", "a01", "a1",
        ];
        v.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(
            v,
            [
                "a01", "a1", "cube", "Cube", "Cube.001", "Cube.9", "Cube.010", "object2",
                "Object10"
            ]
        );
        // Python compares the chunk lists: a leading number sorts before text.
        assert_eq!(natural_cmp("10abc", "abc"), Ordering::Less);
        assert_eq!(natural_cmp("x2", "x10"), Ordering::Less);
        assert_eq!(natural_cmp("x", "x0"), Ordering::Less);
    }

    #[test]
    fn safe_inverse_of_singular_matrix_is_finite() {
        let m = Mat4::from_scale(frap_math::glam::Vec3::new(0.0, 1.0, 1.0));
        let inv = inverted_safe(&m);
        assert!(inv.to_cols_array().iter().all(|x| x.is_finite()));
        let m = Mat4::from_translation(frap_math::glam::Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(inverted_safe(&m).w_axis.x, -1.0);
        let m = Mat4::from_scale_rotation_translation(
            frap_math::glam::Vec3::new(2.0, 3.0, 0.5),
            frap_math::glam::Quat::from_rotation_z(0.7),
            frap_math::glam::Vec3::new(1.0, -2.0, 3.0),
        );
        let id = matmul(&inverted_safe(&m), &m);
        assert!(id.abs_diff_eq(Mat4::IDENTITY, 1e-6));
        assert!(mul_m4(&m, &inverted_safe(&m)).abs_diff_eq(Mat4::IDENTITY, 1e-6));
    }
}
