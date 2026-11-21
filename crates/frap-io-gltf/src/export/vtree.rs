// SPDX-License-Identifier: GPL-3.0-or-later
//! The add-on's virtual export tree (ARCHITECTURE §17.2): which objects become nodes, their
//! hierarchy after filtering, and their world matrices.
//!
//! Ported from blender/scripts/addons_core/io_scene_gltf2/blender/exp/tree.py (`VExportTree`:
//! `construct`, `recursive_node_traverse`, `filter`, `recursive_filter_tag`,
//! `recursive_filter`, `node_filter_*_is_kept`, `break_obj_hierarchy`, and from WP 6.8 the
//! armature passes `search_missing_armature`, `check_if_we_can_remove_armature`,
//! `break_bone_hierarchy`, `bake_armature_bone_list`, `add_leaf_bones`, `get_all_bones`,
//! `get_root_bones_uuid`):
//!
//! - Roots are the scene's objects without a parent, in **natural, case-insensitive** name order
//!   (`_natural_sort_key`: `Object2` < `Object10`, every Unicode decimal digit counting),
//!   ties in the order of `scene_eval.objects` (Python's sort is stable): the evaluated
//!   scene's objects, which are the scene's (`scene_objects`: the master collection's, then
//!   each child collection depth first; verified equal on 5.2.4 with nested, shared, excluded
//!   and hidden collections, objects linked twice and objects added by operators, WP 7.7).
//! - Children are every object of `bpy.data.objects` whose parent is the node, even objects that
//!   are not in the exported scene (the add-on builds `blender_children` from `bpy.data.objects`),
//!   in the same natural order (ties in `bpy.data.objects` order), then (armatures) the root bones by name, (bones) the child bones
//!   by name and the objects parented to the bone (`parent_type = 'BONE'`) by name. A bone node
//!   is `armature.matrix_world @ bone.matrix_local` (`export_rest_position_armature`, the
//!   default) or `@ pose_bone.matrix`, times the Y-up basis change; an object parented to a bone
//!   is moved from the pose to the rest pose of its bone (the `delta` of `recursive_node_traverse`,
//!   passed on to its descendants).
//! - Filters (`use_selection`, `use_visible`, `use_renderable`, `collection`,
//!   `use_active_collection[_with_nested]`, the `glTF_not_exported` collection, cameras and
//!   lights, `export_def_bones`, `export_armature_object_remove`) drop a node; its kept
//!   descendants move to the nearest kept ancestor (appended to its children) or become roots
//!   (appended to the roots). Bones are filtered with their armature object's selection and
//!   visibility.
//! - `matrix_world` is Blender's `parent.matrix_world @ matrix_parent_inverse @ matrix_basis`.
//!
//! Cameras and lights are kept with `export_cameras`/`export_lights` and their world matrix
//! gets the add-on's correction rotation (`cameras.rs`, WP 3.4). Not supported: collection
//! hierarchies (`export_hierarchy_full_collections`) and instancing (later milestones).

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use frap_math::glam::{Mat4, Vec3, Vec4};
use frap_math::matrix::{ObjectTransform, RotationMode, RotationValues};
use frap_rna::{ArmatureId, CollectionId, IdRef, ObjectId};
use frap_scene::id::{Armature, Collection, ModifierSettings, Object};
use frap_scene::{BaseFlag, Context, LayerCollection, Main};
use smol_str::SmolStr;

use crate::export::ExportSettings;

/// Name of the add-on's special collection whose objects are never exported
/// (`BLENDER_GLTF_SPECIAL_COLLECTION`).
pub const SPECIAL_COLLECTION: &str = "glTF_not_exported";

/// `VExportNode.blender_type`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum VType {
    /// `OBJECT` (meshes, empties, point clouds)
    Object,
    /// `ARMATURE`
    Armature,
    /// `BONE` (also the leaf bones of `export_leaf_bone`)
    Bone,
    /// `CAMERA`
    Camera,
    /// `LIGHT`
    Light,
}

/// One node of the export tree.
#[derive(Clone, Debug)]
pub struct VNode {
    /// The object (for a bone: its armature object).
    pub object: IdRef<ObjectId>,
    /// Parent node after filtering.
    pub parent: Option<usize>,
    /// Children after filtering, in export order.
    pub children: Vec<usize>,
    /// `matrix_world` (Blender space; bones include the Y-up basis change).
    pub matrix_world: Mat4,
    /// Survived the filters.
    pub keep: bool,
    /// `blender_type`.
    pub kind: VType,
    /// `blender_bone.name` (bones; `None` for leaf bones).
    pub bone: Option<SmolStr>,
    /// `armature`: a bone's armature node, or the armature node a skinned object (ARMATURE
    /// modifier) is deformed by.
    pub armature: Option<usize>,
    /// `use_deform` of a bone.
    pub use_deform: bool,
    /// `parent_bone_uuid`: the bone node a bone or object hangs from.
    pub parent_bone: Option<usize>,
    /// `leaf_reference`: the bone a leaf bone ends (`export_leaf_bone`).
    pub leaf_reference: Option<usize>,
    /// `matrix_world_tail` (bones, `export_leaf_bone`).
    pub matrix_world_tail: Option<Mat4>,
    /// `original_matrix_world`: the pose-space world matrix of an object moved to the rest pose
    /// of its parent bone (and of its descendants).
    pub original_matrix_world: Option<Mat4>,
    /// `arma_exported` (`export_armature_object_remove`: the armature passed the filters
    /// before being removed).
    pub arma_exported: bool,
    /// `all_bones` of an armature (`get_all_bones`, cached at its first call).
    pub all_bones: Option<Vec<usize>>,
    /// `root_bones_uuid` of an armature (`get_root_bones_uuid`).
    pub root_bones: Option<Vec<usize>>,
}

impl VNode {
    fn new(object: IdRef<ObjectId>, kind: VType, parent: Option<usize>) -> VNode {
        VNode {
            object,
            parent,
            children: Vec::new(),
            matrix_world: Mat4::IDENTITY,
            keep: true,
            kind,
            bone: None,
            armature: None,
            use_deform: false,
            parent_bone: None,
            leaf_reference: None,
            matrix_world_tail: None,
            original_matrix_world: None,
            arma_exported: false,
            all_bones: None,
            root_bones: None,
        }
    }
}

/// The export tree of one scene.
#[derive(Clone, Debug, Default)]
pub struct VTree {
    /// Every node, in construction (pre-)order; dropped nodes stay with `keep == false`.
    pub nodes: Vec<VNode>,
    /// Root nodes in export order.
    pub roots: Vec<usize>,
    /// `gltf_armature_object_remove` as the tree was built (`check_if_we_can_remove_armature`
    /// may have turned it off).
    pub armature_object_remove: bool,
    /// `check_if_we_can_remove_armature` turned `export_armature_object_remove` off.
    pub armature_remove_refused: bool,
}

/// The first code point (digit zero) of every run of ten Unicode decimal digits (general
/// category `Nd`, which is what Python's `re` matches as `\d` in a `str` pattern), Unicode 15.1
/// (Blender 5.2.4's Python 3.13, `unicodedata.unidata_version`; every `Nd` character is in one
/// of these runs, `unicodedata.decimal(zero + k) == k`).
const ND_ZEROS: [u32; 68] = [
    0x30, 0x660, 0x6F0, 0x7C0, 0x966, 0x9E6, 0xA66, 0xAE6, 0xB66, 0xBE6, 0xC66, 0xCE6, 0xD66,
    0xDE6, 0xE50, 0xED0, 0xF20, 0x1040, 0x1090, 0x17E0, 0x1810, 0x1946, 0x19D0, 0x1A80, 0x1A90,
    0x1B50, 0x1BB0, 0x1C40, 0x1C50, 0xA620, 0xA8D0, 0xA900, 0xA9D0, 0xA9F0, 0xAA50, 0xABF0, 0xFF10,
    0x104A0, 0x10D30, 0x11066, 0x110F0, 0x11136, 0x111D0, 0x112F0, 0x11450, 0x114D0, 0x11650,
    0x116C0, 0x11730, 0x118E0, 0x11950, 0x11C50, 0x11D50, 0x11DA0, 0x11F50, 0x16A60, 0x16AC0,
    0x16B50, 0x1D7CE, 0x1D7D8, 0x1D7E2, 0x1D7EC, 0x1D7F6, 0x1E140, 0x1E2F0, 0x1E4F0, 0x1E950,
    0x1FBF0,
];

/// The value of a Unicode decimal digit (`None` for any other character).
fn decimal_digit(c: char) -> Option<u32> {
    let c = c as u32;
    let i = ND_ZEROS.partition_point(|&z| z <= c);
    let zero = *ND_ZEROS.get(i.checked_sub(1)?)?;
    (c - zero < 10).then_some(c - zero)
}

/// `_natural_sort_key` order (`[int(c) if c.isdigit() else c.lower() for c in
/// re.split(r'(\d+)', name)]`): names split into text and digit runs, text compared lower-cased
/// (code point order), digit runs compared as integers. Every Unicode decimal digit counts
/// (`x١٠٠` is `x100`, `x١` sorts before `x2`); equal keys (`a01` / `a1`, `Cube` / `cube`) keep
/// the order of the list being sorted (Python's sort is stable).
///
/// The add-on raises `ValueError` for a name with a text run made only of digits that `\d`
/// does not match (`a1²`: `'²'.isdigit()` holds and `int('²')` fails), so its export fails;
/// frap compares such a run as text (docs/deviations.md §1).
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    /// A chunk: whether it is a digit run, and its code points (text, lower-cased) or digit
    /// values without leading zeros (a number).
    type Chunk = (bool, Vec<u32>);
    fn key(s: &str) -> Vec<Chunk> {
        // Alternating text / number chunks, starting and ending with (possibly empty) text.
        let mut out: Vec<Chunk> = vec![(false, Vec::new())];
        let mut text = String::new();
        let flush = |text: &mut String, out: &mut Vec<Chunk>| {
            let last = out.last_mut().expect("never empty");
            last.1 = text.to_lowercase().chars().map(u32::from).collect();
            text.clear();
        };
        for c in s.chars() {
            match decimal_digit(c) {
                Some(d) => {
                    if !out.last().is_some_and(|l| l.0) {
                        flush(&mut text, &mut out);
                        out.push((true, Vec::new()));
                    }
                    let last = out.last_mut().expect("never empty");
                    if !(d == 0 && last.1.is_empty()) {
                        last.1.push(d);
                    }
                }
                None => {
                    if out.last().is_some_and(|l| l.0) {
                        out.push((false, Vec::new()));
                    }
                    text.push(c);
                }
            }
        }
        if out.last().is_some_and(|l| l.0) {
            out.push((false, Vec::new()));
        }
        flush(&mut text, &mut out);
        out
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
/// @ matrix_basis`, associated as `BKE_object_where_is_calc` does (`mul_m4_series`). A
/// `parent_type = 'BONE'` child of an armature reads the pose of its bone
/// (`frap_scene::id::transform::parent_matrix_in`, WP 6.8); other parent types are `OBJECT`.
pub fn matrix_world(main: &Main, ob: &Object) -> Mat4 {
    fn rec(main: &Main, ob: &Object, depth: usize) -> Mat4 {
        let basis = matrix_basis(ob);
        // Bounded: a parent cycle (impossible in Blender) cannot recurse forever.
        match ob.parent.and_then(|p| main.objects.get(p)) {
            Some(p) if depth < main.objects.len() => {
                let pw = rec(main, p, depth + 1);
                let pm = if ob.parent_type == "BONE" {
                    frap_scene::id::transform::parent_matrix_in(main, ob, p, pw)
                } else {
                    pw
                };
                mul_m4(&mul_m4(&pm, &ob.matrix_parent_inverse), &basis)
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
    // `determinant_m3`: expanded along the first column (`a1 * det2(b2, b3, c2, c3) - b1 *
    // det2(a2, a3, c2, c3) + c1 * det2(a2, a3, b2, b3)`), as Blender rounds it.
    a1 * (b2 * c3 - b3 * c2) - b1 * (a2 * c3 - a3 * c2) + c1 * (a2 * b3 - a3 * b2)
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
/// `PSEUDOINVERSE_EPSILON` (1e-8) added to its diagonal first (and becomes the identity, with
/// determinant 1, if it is still singular).
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
            src = m4(&Mat4::IDENTITY);
            det = 1.0;
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

/// The bone options of the export settings, as the add-on derives them
/// (`ExportGLTF2.execute`): `export_def_bones` only with `export_skins`, and not when
/// animations are exported without `export_force_sampling`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct BoneSettings {
    /// `gltf_skins`
    pub skins: bool,
    /// `gltf_def_bones`
    pub def_bones: bool,
    /// `gltf_flatten_bones_hierarchy`
    pub flatten_bones: bool,
    /// `gltf_armature_object_remove` (before `check_if_we_can_remove_armature`)
    pub armature_object_remove: bool,
    /// `gltf_leaf_bone`
    pub leaf_bone: bool,
    /// `gltf_rest_position_armature`
    pub rest_position: bool,
}

impl BoneSettings {
    /// The bone options of `s`.
    pub fn new(s: &ExportSettings) -> BoneSettings {
        let p = &s.props;
        let skins = p.export_skins.value;
        let def_bones = p.export_def_bones.value
            && skins
            && !(p.export_animations.value && !p.export_force_sampling.value);
        BoneSettings {
            skins,
            def_bones,
            flatten_bones: p.export_hierarchy_flatten_bones.value,
            armature_object_remove: p.export_armature_object_remove.value,
            leaf_bone: p.export_leaf_bone.value,
            rest_position: p.export_rest_position_armature.value,
        }
    }
}

/// `axis_basis_change`: identity, or with `export_yup` the rotation taking Blender's +Z to
/// glTF's +Y (rows `(1,0,0,0) (0,0,1,0) (0,-1,0,0) (0,0,0,1)`).
pub fn axis_basis_change(export_yup: bool) -> Mat4 {
    if export_yup {
        Mat4::from_cols(
            Vec4::new(1.0, 0.0, 0.0, 0.0),
            Vec4::new(0.0, 0.0, -1.0, 0.0),
            Vec4::new(0.0, 1.0, 0.0, 0.0),
            Vec4::new(0.0, 0.0, 0.0, 1.0),
        )
    } else {
        Mat4::IDENTITY
    }
}

/// `Matrix.Translation(v)`.
fn translation(v: Vec3) -> Mat4 {
    Mat4::from_translation(v)
}

/// The armature data of an ARMATURE object.
pub fn armature_of<'m>(main: &'m Main, ob: &Object) -> Option<&'m Armature> {
    ob.data
        .and_then(|d| d.typed::<ArmatureId>())
        .and_then(|a| main.armatures.get(a))
}

/// What the tree reads of an armature: per bone index, the rest matrix (`Bone.matrix_local`,
/// `EditBone.tail`) and the pose (`PoseBone.matrix`, `PoseBone.tail`).
pub struct ArmatureMats {
    /// `armature_finalize_restpose` per bone.
    pub rest: Vec<frap_scene::edit_armature::rest::RestBone>,
    /// `BKE_pose_where_is` per bone.
    pub pose: Vec<frap_scene::edit_armature::posing::PoseMats>,
}

impl ArmatureMats {
    /// The matrices of `arm` posed by `ob.pose`.
    pub fn new(arm: &Armature, ob: &Object) -> ArmatureMats {
        let rest = frap_scene::edit_armature::rest::rest_pose(arm);
        let pose = frap_scene::edit_armature::posing::where_is(
            arm,
            &rest,
            ob.pose.as_ref(),
            arm.pose_position == "REST",
        );
        ArmatureMats { rest, pose }
    }
}

/// The last ARMATURE modifier of `ob` (`{m.type: m for m in modifiers}["ARMATURE"]`) and its
/// object.
pub fn armature_modifier_object(ob: &Object) -> Option<Option<IdRef<ObjectId>>> {
    ob.modifiers.iter().rev().find_map(|m| match &m.settings {
        ModifierSettings::Armature(a) => Some(a.object.and_then(|o| o.typed::<ObjectId>())),
        _ => None,
    })
}

/// A node to create (`recursive_node_traverse`'s arguments).
enum Item<'m> {
    Object(IdRef<ObjectId>, &'m Object),
    Bone {
        arm_ob: IdRef<ObjectId>,
        bone: usize,
        arm_node: usize,
    },
}

/// Builds the export tree of the context scene.
pub fn build(main: &Main, ctx: &Context, s: &ExportSettings) -> VTree {
    let b = BoneSettings::new(s);
    build_ex(main, ctx, s, b.armature_object_remove)
}

/// [`build`] with `gltf_armature_object_remove` given (an earlier scene's
/// `check_if_we_can_remove_armature` may have turned it off for the rest of the export).
pub fn build_ex(
    main: &Main,
    ctx: &Context,
    s: &ExportSettings,
    armature_object_remove: bool,
) -> VTree {
    let bs = BoneSettings {
        armature_object_remove,
        ..BoneSettings::new(s)
    };
    let abc = axis_basis_change(s.export_yup);
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
    fn sorted(mut v: Vec<(IdRef<ObjectId>, &Object)>) -> Vec<(IdRef<ObjectId>, &Object)> {
        v.sort_by(|a, b| natural_cmp(&a.1.header.name, &b.1.header.name));
        v
    }

    let mut tree = VTree {
        armature_object_remove: bs.armature_object_remove,
        ..VTree::default()
    };
    let mut visited = BTreeSet::new();
    let mut mats: BTreeMap<IdRef<ObjectId>, ArmatureMats> = BTreeMap::new();
    // `armature_needed`: skinned objects not parented to their armature (searched by name).
    let mut armature_needed: Vec<(usize, SmolStr)> = Vec::new();
    // Construction (`recursive_node_traverse`), pre-order.
    let mut stack: Vec<(Item, Option<usize>, bool)> = roots
        .iter()
        .rev()
        .map(|(r, o)| (Item::Object(*r, o), None, false))
        .collect();
    while let Some((item, parent, delta)) = stack.pop() {
        let i = tree.nodes.len();
        let mut new_delta = false;
        let node = match item {
            Item::Object(r, ob) => {
                if !visited.insert(r) {
                    continue;
                }
                let kind = match ob.object_type() {
                    "ARMATURE" => VType::Armature,
                    "CAMERA" => VType::Camera,
                    "LIGHT" => VType::Light,
                    _ => VType::Object,
                };
                let mut n = VNode::new(r, kind, parent);
                if kind == VType::Object
                    && let Some(Some(mod_ob)) = armature_modifier_object(ob)
                {
                    match parent {
                        Some(p) if tree.nodes[p].kind == VType::Armature => n.armature = Some(p),
                        _ => {
                            if let Some(name) = main.objects.name_of(mod_ob) {
                                armature_needed.push((i, SmolStr::new(name)));
                            }
                        }
                    }
                }
                if let Some(p) = parent
                    && tree.nodes[p].kind == VType::Bone
                {
                    n.parent_bone = Some(p);
                }
                let mut mw = matrix_world(main, ob);
                // Rest position: an object parented to a bone keeps its offset to the bone's
                // pose, applied to the bone's rest matrix.
                if let Some(p) = parent
                    && tree.nodes[p].kind == VType::Bone
                    && bs.rest_position
                    && let (Some(arm_node), Some(bone)) =
                        (tree.nodes[p].armature, tree.nodes[p].bone.clone())
                {
                    let arm_ob = tree.nodes[arm_node].object;
                    let arm_mw = tree.nodes[arm_node].matrix_world;
                    if let Some(am) = mats.get(&arm_ob)
                        && let Some(arm) =
                            main.objects.get(arm_ob).and_then(|o| armature_of(main, o))
                        && let Some(bi) = arm.bone_index(&bone)
                    {
                        let pose = matmul(&matmul(&arm_mw, &am.pose[bi].pose_mat), &abc);
                        let rest = matmul(&matmul(&arm_mw, &am.rest[bi].arm_mat), &abc);
                        let d = matmul(&inverted_safe(&pose), &mw);
                        n.original_matrix_world = Some(mw);
                        mw = matmul(&rest, &d);
                        new_delta = true;
                    }
                }
                // Cameras and lights get the add-on's Y-up correction (WP 3.4, `cameras.rs`).
                n.matrix_world = crate::export::cameras::correct_matrix_world(s, ob, mw);
                // Children: objects (not parented to a bone), then the root bones.
                let kids: Vec<(IdRef<ObjectId>, &Object)> = children_of
                    .get(&r)
                    .map(|v| {
                        v.iter()
                            .filter(|(_, c)| {
                                !(matches!(c.parent_type.as_str(), "BONE" | "BONE_RELATIVE")
                                    && !c.parent_bone.is_empty())
                            })
                            .copied()
                            .collect()
                    })
                    .unwrap_or_default();
                let mut items: Vec<Item> = sorted(kids)
                    .into_iter()
                    .map(|(cr, co)| Item::Object(cr, co))
                    .collect();
                if kind == VType::Armature
                    && let Some(arm) = armature_of(main, ob)
                {
                    mats.entry(r).or_insert_with(|| ArmatureMats::new(arm, ob));
                    let mut root_bones: Vec<usize> = (0..arm.bones.len())
                        .filter(|&k| arm.bones[k].parent.is_none())
                        .collect();
                    root_bones
                        .sort_by(|&a, &b| natural_cmp(&arm.bones[a].name, &arm.bones[b].name));
                    items.extend(root_bones.into_iter().map(|bone| Item::Bone {
                        arm_ob: r,
                        bone,
                        arm_node: i,
                    }));
                }
                for it in items.into_iter().rev() {
                    stack.push((it, Some(i), new_delta || delta));
                }
                n
            }
            Item::Bone {
                arm_ob,
                bone,
                arm_node,
            } => {
                let Some(ob) = main.objects.get(arm_ob) else {
                    continue;
                };
                let Some(arm) = armature_of(main, ob) else {
                    continue;
                };
                let Some(am) = mats.get(&arm_ob) else {
                    continue;
                };
                let mut n = VNode::new(arm_ob, VType::Bone, parent);
                n.bone = Some(arm.bones[bone].name.clone());
                n.use_deform = arm.bones[bone].use_deform;
                n.armature = Some(arm_node);
                if let Some(p) = parent
                    && tree.nodes[p].kind == VType::Bone
                {
                    n.parent_bone = Some(p);
                }
                let arm_mw = tree.nodes[arm_node].matrix_world;
                let mw = if bs.rest_position {
                    matmul(&arm_mw, &am.rest[bone].arm_mat)
                } else {
                    if bs.leaf_bone {
                        let tail = matmul(&arm_mw, &translation(am.pose[bone].tail));
                        n.matrix_world_tail = Some(matmul(&tail, &abc));
                    }
                    matmul(&arm_mw, &am.pose[bone].pose_mat)
                };
                n.matrix_world = matmul(&mw, &abc);
                // Children: the child bones, then the objects parented to this bone.
                let mut child_bones: Vec<usize> = (0..arm.bones.len())
                    .filter(|&k| arm.bones[k].parent == Some(bone))
                    .collect();
                child_bones.sort_by(|&a, &b| natural_cmp(&arm.bones[a].name, &arm.bones[b].name));
                let mut items: Vec<Item> = child_bones
                    .into_iter()
                    .map(|cb| Item::Bone {
                        arm_ob,
                        bone: cb,
                        arm_node,
                    })
                    .collect();
                let bone_kids: Vec<(IdRef<ObjectId>, &Object)> = children_of
                    .get(&arm_ob)
                    .map(|v| {
                        v.iter()
                            .filter(|(_, c)| {
                                c.parent_type == "BONE" && c.parent_bone == arm.bones[bone].name
                            })
                            .copied()
                            .collect()
                    })
                    .unwrap_or_default();
                items.extend(
                    sorted(bone_kids)
                        .into_iter()
                        .map(|(cr, co)| Item::Object(cr, co)),
                );
                for it in items.into_iter().rev() {
                    stack.push((it, Some(i), delta));
                }
                n
            }
        };
        let mut node = node;
        if delta && let Some(p) = parent {
            let pose_parent = tree.nodes[p]
                .original_matrix_world
                .unwrap_or(tree.nodes[p].matrix_world);
            let rest_parent = tree.nodes[p].matrix_world;
            let d = matmul(&inverted_safe(&pose_parent), &node.matrix_world);
            node.original_matrix_world = Some(node.matrix_world);
            node.matrix_world = matmul(&rest_parent, &d);
        }
        tree.nodes.push(node);
        match parent {
            Some(p) => tree.nodes[p].children.push(i),
            None => tree.roots.push(i),
        }
    }

    // `search_missing_armature`: the first armature node with the modifier object's name.
    for (i, name) in armature_needed {
        tree.nodes[i].armature = (0..tree.nodes.len()).find(|&a| {
            tree.nodes[a].kind == VType::Armature
                && main
                    .objects
                    .get(tree.nodes[a].object)
                    .is_some_and(|o| o.header.name == name)
        });
    }
    // `check_if_we_can_remove_armature`: not with several root bones.
    if tree.armature_object_remove {
        let refuse = (0..tree.nodes.len())
            .filter(|&a| tree.nodes[a].kind == VType::Armature)
            .any(|a| tree.root_bones_of(a, bs.def_bones).len() > 1);
        if refuse {
            tree.armature_object_remove = false;
            tree.armature_remove_refused = true;
        }
    }

    // `filter_tag`.
    let filters = Filters::new(main, ctx, s);
    for i in 0..tree.nodes.len() {
        let keep = filters.not_inheritable_node(&tree, i, &bs) && {
            let r = tree.nodes[i].object;
            match main.objects.get(r) {
                Some(ob) if filters.inheritable(r, ob) => {
                    if tree.armature_object_remove && tree.nodes[i].kind == VType::Armature {
                        tree.nodes[i].arma_exported = true;
                        false
                    } else {
                        true
                    }
                }
                _ => false,
            }
        };
        tree.nodes[i].keep = keep;
    }
    filter(&mut tree);
    if bs.flatten_bones {
        tree.break_bone_hierarchy();
    }
    tree.bake_armature_bone_list(main, &bs, abc);
    if s.export_hierarchy_flatten_objs {
        flatten(&mut tree);
    }
    tree
}

impl Filters<'_> {
    /// `node_filter_not_inheritable_is_kept` of a tree node: cameras and lights by their
    /// options; with `export_def_bones` a non-deforming bone only when objects hang from it.
    fn not_inheritable_node(&self, tree: &VTree, i: usize, bs: &BoneSettings) -> bool {
        let n = &tree.nodes[i];
        match n.kind {
            VType::Camera => crate::export::cameras::exported(self.s),
            VType::Light => crate::export::lights::exported(self.s),
            VType::Bone if bs.def_bones && !n.use_deform => n
                .children
                .iter()
                .any(|&c| tree.nodes[c].kind != VType::Bone),
            _ => true,
        }
    }
}

/// `recursive_filter`: drops nodes with `keep == false`, moving kept descendants up. A dropped
/// armature of `export_armature_object_remove` also loses its parent (its data is still read).
fn filter(tree: &mut VTree) {
    fn rec(tree: &mut VTree, i: usize, parent_kept: Option<usize>) {
        let children = tree.nodes[i].children.clone();
        let new_parent_kept;
        if !tree.nodes[i].keep {
            new_parent_kept = parent_kept;
            match tree.nodes[i].parent {
                Some(p) => {
                    tree.nodes[p].children.retain(|c| *c != i);
                    if tree.nodes[i].kind == VType::Armature && tree.armature_object_remove {
                        tree.nodes[i].parent = None;
                    }
                }
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

/// `break_obj_hierarchy` (`export_hierarchy_flatten_objs`): every node but the bones becomes a
/// root, in construction order; a skinned object stays the child of its armature.
fn flatten(tree: &mut VTree) {
    for i in 0..tree.nodes.len() {
        if !tree.exists(i) || tree.nodes[i].kind == VType::Bone {
            continue;
        }
        if tree.nodes[i].armature.is_some() && tree.nodes[i].parent == tree.nodes[i].armature {
            continue;
        }
        if let Some(p) = tree.nodes[i].parent.take() {
            tree.nodes[p].children.retain(|c| *c != i);
            tree.roots.push(i);
        }
    }
}

impl VTree {
    /// The node is still in the add-on's `nodes` after `remove_filtered_nodes`: kept, or an
    /// armature removed by `export_armature_object_remove`.
    pub fn exists(&self, i: usize) -> bool {
        let n = &self.nodes[i];
        n.keep || (n.kind == VType::Armature && self.armature_object_remove)
    }

    /// The bone children of an armature node (`get_root_bones_uuid` without the cache), only
    /// the deforming ones with `def_bones`.
    fn root_bones_of(&self, arm: usize, def_bones: bool) -> Vec<usize> {
        self.nodes[arm]
            .children
            .iter()
            .copied()
            .filter(|&c| self.nodes[c].kind == VType::Bone)
            .filter(|&c| !def_bones || self.nodes[c].use_deform)
            .collect()
    }

    /// `get_all_bones`: the bones under an armature node, pre-order, cached at the first call.
    fn ensure_all_bones(&mut self, arm: usize) -> Vec<usize> {
        if let Some(b) = &self.nodes[arm].all_bones {
            return b.clone();
        }
        fn rec(t: &VTree, i: usize, out: &mut Vec<usize>, depth: usize) {
            if !t.exists(i) || t.nodes[i].kind != VType::Bone || depth > t.nodes.len() {
                return;
            }
            out.push(i);
            for &c in &t.nodes[i].children {
                rec(t, c, out, depth + 1);
            }
        }
        let mut out = Vec::new();
        if self.nodes[arm].kind == VType::Armature {
            for &c in &self.nodes[arm].children {
                rec(self, c, &mut out, 0);
            }
        }
        self.nodes[arm].all_bones = Some(out.clone());
        out
    }

    /// The bones of an armature node (`get_all_bones`), the skin's joints in order.
    pub fn all_bones(&self, arm: usize) -> &[usize] {
        self.nodes[arm].all_bones.as_deref().unwrap_or(&[])
    }

    /// The root bones of an armature node (`get_root_bones_uuid`, cached by
    /// `bake_armature_bone_list`): its joint children.
    pub fn root_bones(&self, arm: usize) -> &[usize] {
        self.nodes[arm].root_bones.as_deref().unwrap_or(&[])
    }

    /// `break_bone_hierarchy` (`export_hierarchy_flatten_bones`): every bone becomes a child
    /// of its armature node.
    fn break_bone_hierarchy(&mut self) {
        for arm in 0..self.nodes.len() {
            if !self.exists(arm) || self.nodes[arm].kind != VType::Armature {
                continue;
            }
            for bone in self.ensure_all_bones(arm) {
                if let Some(p) = self.nodes[bone].parent
                    && p != arm
                {
                    self.nodes[p].children.retain(|c| *c != bone);
                    self.nodes[bone].parent = Some(arm);
                    self.nodes[arm].children.push(bone);
                    self.nodes[bone].parent_bone = None;
                }
            }
        }
    }

    /// `bake_armature_bone_list`: the leaf bones (`export_leaf_bone`), then each armature's
    /// bone list and root bones.
    fn bake_armature_bone_list(&mut self, main: &Main, bs: &BoneSettings, abc: Mat4) {
        if bs.leaf_bone {
            self.add_leaf_bones(main, bs, abc);
        }
        for arm in 0..self.nodes.len() {
            if !self.exists(arm) || self.nodes[arm].kind != VType::Armature {
                continue;
            }
            self.ensure_all_bones(arm);
            let roots = self.root_bones_of(arm, bs.def_bones);
            self.nodes[arm].root_bones = Some(roots);
        }
    }

    /// `add_leaf_bones`: a `<bone>_leaf` joint at the tail of every bone without children.
    fn add_leaf_bones(&mut self, main: &Main, bs: &BoneSettings, abc: Mat4) {
        if bs.rest_position {
            for b in 0..self.nodes.len() {
                if !self.exists(b) || self.nodes[b].kind != VType::Bone {
                    continue;
                }
                if self.nodes[b]
                    .children
                    .iter()
                    .any(|&c| self.nodes[c].kind == VType::Bone)
                {
                    continue;
                }
                let Some(ob) = main.objects.get(self.nodes[b].object) else {
                    continue;
                };
                let Some(arm) = armature_of(main, ob) else {
                    continue;
                };
                let Some(bone) = self.nodes[b].bone.as_deref().and_then(|n| arm.bone(n)) else {
                    continue;
                };
                let tail = Vec3::from_array(bone.tail_local);
                let m = matmul(&matrix_world(main, ob), &translation(tail));
                self.nodes[b].matrix_world_tail = Some(matmul(&m, &abc));
            }
        }
        let leaves: Vec<usize> = (0..self.nodes.len())
            .filter(|&b| {
                self.exists(b)
                    && self.nodes[b].kind == VType::Bone
                    && self.nodes[b].children.is_empty()
            })
            .collect();
        for b in leaves {
            if bs.def_bones && !self.nodes[b].use_deform {
                continue;
            }
            let Some(tail) = self.nodes[b].matrix_world_tail else {
                continue;
            };
            let i = self.nodes.len();
            let mut n = VNode::new(self.nodes[b].object, VType::Bone, Some(b));
            n.parent_bone = Some(b);
            n.armature = self.nodes[b].armature;
            n.leaf_reference = Some(b);
            n.matrix_world = tail;
            self.nodes.push(n);
            self.nodes[b].children.push(i);
        }
    }

    /// The node's matrix relative to its (kept) parent: `parent.matrix_world.inverted_safe() @
    /// matrix_world`, or the world matrix of a root.
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

    /// Blender 5.2.4 (`followups/natural_digits`): roots `x10`, `x٣`, `x2`, `x１`, `X𝟗`,
    /// `x١٠٠` export as `x１ x2 x٣ X𝟗 x10 x١٠٠` (Arabic-Indic, full-width and mathematical
    /// digits are numbers).
    #[test]
    fn natural_order_counts_every_decimal_digit() {
        let mut v = vec![
            "x10",
            "x\u{663}",
            "x2",
            "x\u{ff11}",
            "X\u{1d7d7}",
            "x\u{661}\u{660}\u{660}",
        ];
        v.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(
            v,
            [
                "x\u{ff11}",
                "x2",
                "x\u{663}",
                "X\u{1d7d7}",
                "x10",
                "x\u{661}\u{660}\u{660}"
            ]
        );
        // Leading zeros of any script are not part of the number.
        assert_eq!(natural_cmp("a\u{660}\u{661}", "a1"), Ordering::Equal);
        assert_eq!(natural_cmp("a\u{6f0}", "a0"), Ordering::Equal);
        // Superscripts are not decimal digits: text.
        assert_eq!(natural_cmp("a\u{b2}", "a2"), Ordering::Greater);
        for z in ND_ZEROS {
            let zero = char::from_u32(z).unwrap();
            assert_eq!(decimal_digit(zero), Some(0));
            assert_eq!(decimal_digit(char::from_u32(z + 9).unwrap()), Some(9));
        }
        assert_eq!(decimal_digit('a'), None);
        assert_eq!(decimal_digit('\u{b2}'), None);
        assert_eq!(decimal_digit('/'), None);
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
