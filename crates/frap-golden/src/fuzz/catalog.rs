// SPDX-License-Identifier: GPL-3.0-or-later
//! What the differential fuzzer draws from: the implemented operators (with the properties it
//! varies) and the RNA writes and function calls (WP 8.7).
//!
//! **Operators** are one line each: `<ctx> <idname>; <prop>; <prop>=<override>; …` where
//! `<ctx>` is `O` (Object Mode, an active mesh object), `E` (mesh Edit Mode) or `A` (either).
//! A listed property is passed with probability ½ (always with a `!` prefix); its values come
//! from the operator's Blender 5.2.4 definition in `spec/blender-5.2/ops/` (enum items, enum
//! flags as sets, booleans, vector lengths, the soft range clamped to a small window), unless
//! the override gives a range `lo:hi` (per component for vectors) or Python literals
//! separated by ` | `. Literals may name the generator's state: `$mod` (a modifier the
//! sequence added), `$obj` (an object name), `$vg` (a vertex group name).
//!
//! **RNA statements** are templates: `<ctx> <python>` with `$f(lo,hi)`, `$i(lo,hi)`, `$b`,
//! `$v3(lo,hi)`, `$b3`, `$c(a|b|…)` and the names above.
//!
//! The operators are chosen to avoid the documented deviations that cases avoid too
//! (`docs/deviations.md`, Golden column "cases …"): no `#@frap`/`#@blender` helpers, no
//! headless-extended picks (`loop_select`, `*_pick`, `rip*`), `select_linked`/
//! `dissolve_limited` always with an explicit `delimit` (D-64), no mesh-data writes in Edit
//! Mode (D-06), no slides without `correct_uv` (D-83), `normals_tools` never in its default
//! `COPY` mode (D-150), the active object only changed in Object Mode (the frap dumper reads
//! an Edit Mode selection history through the response's `edit_mesh` block, which exists only
//! while the context is in Edit Mode). `known.rs` lists the rest.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde_json::Value as Json;

/// Where an entry applies.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Ctx {
    /// Object Mode.
    Object,
    /// Mesh Edit Mode.
    Edit,
    /// Either mode.
    Any,
}

impl Ctx {
    fn parse(s: &str) -> Option<Ctx> {
        Some(match s {
            "O" => Ctx::Object,
            "E" => Ctx::Edit,
            "A" => Ctx::Any,
            _ => return None,
        })
    }

    /// Whether the entry applies in Edit Mode (`edit`) or Object Mode.
    pub fn applies(self, edit: bool) -> bool {
        match self {
            Ctx::Object => !edit,
            Ctx::Edit => edit,
            Ctx::Any => true,
        }
    }
}

/// How one property's values are drawn.
#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    /// `True`/`False`, or a tuple of `n` of them.
    Bool { n: usize },
    /// An integer in `lo..=hi`, or a tuple of `n`.
    Int { lo: i64, hi: i64, n: usize },
    /// A float in `[lo, hi]` with two decimals, or a tuple of `n`.
    Float { lo: f64, hi: f64, n: usize },
    /// One enum item.
    Enum(Vec<String>),
    /// A non-empty subset of the items, as a set literal.
    EnumFlag(Vec<String>),
    /// One of these Python literals.
    Literal(Vec<String>),
}

/// One varied property.
#[derive(Clone, Debug, PartialEq)]
pub struct PropGen {
    /// RNA identifier.
    pub name: String,
    /// Always passed (`!`).
    pub always: bool,
    /// Values.
    pub kind: Kind,
}

/// One operator of the catalogue.
#[derive(Clone, Debug, PartialEq)]
pub struct OpEntry {
    /// `ns.op`.
    pub idname: String,
    /// Mode.
    pub ctx: Ctx,
    /// Varied properties.
    pub props: Vec<PropGen>,
}

/// One RNA statement template.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RnaEntry {
    /// Mode.
    pub ctx: Ctx,
    /// The template.
    pub template: String,
}

/// The operator lines (see the module docs).
pub const OPERATORS: &[&str] = &[
    // --- mesh: primitives (both modes: in Edit Mode they add into the edit mesh) ---------
    "A mesh.primitive_plane_add; size=0.5:3; location=-2:2; rotation=-1:1",
    "A mesh.primitive_cube_add; size=0.5:3; location=-2:2; rotation=-1:1",
    "A mesh.primitive_circle_add; vertices=3:16; radius=0.5:2; fill_type; location=-2:2",
    "A mesh.primitive_uv_sphere_add; segments=3:12; ring_count=3:8; radius=0.5:2; location=-2:2",
    "A mesh.primitive_ico_sphere_add; subdivisions=1:2; radius=0.5:2; location=-2:2",
    "A mesh.primitive_cylinder_add; vertices=3:12; radius=0.5:2; depth=0.5:3; end_fill_type; location=-2:2",
    "A mesh.primitive_cone_add; vertices=3:12; radius1=0:2; radius2=0:2; depth=0.5:3; end_fill_type",
    "A mesh.primitive_torus_add; major_segments=3:12; minor_segments=3:8; major_radius=0.5:2; minor_radius=0.1:0.5",
    "A mesh.primitive_grid_add; x_subdivisions=1:6; y_subdivisions=1:6; size=0.5:3; location=-2:2",
    "A mesh.primitive_monkey_add; size=0.5:3; location=-2:2",
    // --- mesh: selection ----------------------------------------------------------------
    "E mesh.select_all; !action",
    "E mesh.select_mode; use_extend; use_expand; !type; action",
    "E mesh.select_linked; !delimit",
    "E mesh.select_more; use_face_step",
    "E mesh.select_less; use_face_step",
    "E mesh.select_edge_loop_multi; delimit_edge_loop",
    "E mesh.select_edge_ring_multi",
    "E mesh.select_non_manifold; extend; use_wire; use_boundary; use_multi_face; use_non_contiguous; use_verts",
    "E mesh.select_loose; extend",
    "E mesh.select_face_by_sides; number=3:6; type; extend",
    "E mesh.select_random; ratio=0:1; !seed=0:1000; action",
    "E mesh.faces_select_linked_flat; sharpness=0:1",
    "E mesh.loop_to_region; select_bigger",
    "E mesh.region_to_loop",
    "E mesh.select_nth; skip=1:3; nth=1:2; offset=0:3",
    "E mesh.select_boundary_loop_multi; extend",
    "E mesh.select_similar; type; compare; threshold=0:0.5",
    "E mesh.select_similar_region",
    "E mesh.select_interior_faces",
    "E mesh.select_mirror; axis; extend",
    "E mesh.select_axis; orientation=LOCAL | GLOBAL; sign; axis; threshold=0:0.1",
    "E mesh.select_ungrouped; extend",
    "E mesh.select_by_pole_count; pole_count=2:6; type; extend; exclude_nonmanifold",
    "E mesh.select_next_item",
    "E mesh.select_prev_item",
    "E mesh.edges_select_sharp; sharpness=0.1:3",
    "E mesh.hide; unselected",
    "E mesh.reveal; select",
    // --- mesh: extrude and topology tools ---------------------------------------------------
    "E mesh.extrude_region; use_normal_flip",
    "E mesh.extrude_region_move; !TRANSFORM_OT_translate={'value': (0, 0, 1)} | {'value': (0.5, -0.25, 0)}",
    "E mesh.extrude_context; use_normal_flip",
    "E mesh.extrude_faces_indiv",
    "E mesh.extrude_edges_indiv",
    "E mesh.extrude_verts_indiv",
    "E mesh.extrude_repeat; steps=1:3; offset=-1:1",
    "E mesh.extrude_faces_move; !TRANSFORM_OT_shrink_fatten={'value': 0.5} | {'value': -0.2}",
    "E mesh.extrude_edges_move; !TRANSFORM_OT_translate={'value': (0, 0, 1)}",
    "E mesh.extrude_vertices_move; !TRANSFORM_OT_translate={'value': (0.5, 0, 0)}",
    "E mesh.inset; thickness=0:0.4; depth=-0.3:0.3; use_boundary; use_even_offset; use_relative_offset; use_edge_rail; use_outset; use_select_inset; use_individual",
    "E mesh.bevel; offset=0:0.4; segments=1:4; profile=0.1:1; affect; clamp_overlap; loop_slide; mark_seam; mark_sharp",
    "E mesh.subdivide; number_cuts=1:2; smoothness=0:1; ngon; quadcorner; fractal=0:1; seed=0:100",
    "E mesh.subdivide_edgering; number_cuts=1:3; interpolation; smoothness=0:1",
    "E mesh.unsubdivide; iterations=1:3",
    "E mesh.offset_edge_loops; use_cap_endpoint",
    "E mesh.merge; type=CENTER | CURSOR | COLLAPSE; uvs",
    "E mesh.remove_doubles; threshold=0:0.5; use_unselected; use_sharp_edge_from_normals",
    "E mesh.delete; type",
    "E mesh.dissolve_verts; use_face_split; use_boundary_tear",
    "E mesh.dissolve_edges; use_verts; use_face_split",
    "E mesh.dissolve_faces; use_verts",
    "E mesh.dissolve_mode; use_verts; use_face_split; use_boundary_tear",
    "E mesh.dissolve_limited; angle_limit=0:1; use_dissolve_boundaries; !delimit",
    "E mesh.delete_loose; use_verts; use_edges; use_faces",
    "E mesh.delete_edgeloop; use_face_split",
    "E mesh.dissolve_degenerate; threshold=0:0.1",
    "E mesh.edge_collapse",
    "E mesh.edge_face_add",
    "E mesh.bridge_edge_loops; type; use_merge; merge_factor=0:1; twist_offset=-1:1; number_cuts=0:2",
    "E mesh.fill; use_beauty",
    "E mesh.fill_grid; span=1:3; offset=0:3; use_interp_simple",
    "E mesh.vert_connect",
    "E mesh.vert_connect_path",
    "E mesh.fill_holes; sides=0:6",
    "E mesh.beautify_fill; angle_limit=0:3",
    "E mesh.vert_connect_concave",
    "E mesh.vert_connect_nonplanar; angle_limit=0:1",
    "E mesh.face_split_by_edges",
    "E mesh.edge_rotate; use_ccw",
    "E mesh.bisect; !plane_co=-0.5:0.5; !plane_no=(0, 0, 1) | (1, 0, 0) | (0.6, 0, 0.8); use_fill; clear_inner; clear_outer",
    "E mesh.convex_hull; delete_unused; use_existing_faces; make_holes; join_triangles",
    "E mesh.flip_normals",
    "E mesh.normals_make_consistent; inside",
    "E mesh.faces_shade_smooth",
    "E mesh.faces_shade_flat",
    "E mesh.mark_sharp; clear; use_verts",
    "E mesh.mark_seam; clear",
    "E mesh.set_sharpness_by_angle; angle=0.1:3; extend",
    "E mesh.split",
    "E mesh.edge_split; type",
    "E mesh.separate; !type=SELECTED | LOOSE",
    "E mesh.duplicate",
    "E mesh.duplicate_move; !TRANSFORM_OT_translate={'value': (1, 0, 0)} | {'value': (0, 0.5, 0.5)}",
    "E mesh.spin; steps=1:8; angle=-3:3; !center=-1:1; !axis=(0, 0, 1) | (1, 0, 0); dupli; use_auto_merge; use_normal_flip",
    "E mesh.screw; steps=1:8; turns=1:2; !center=-1:1; !axis=(0, 0, 1) | (0, 1, 0)",
    "E mesh.solidify; thickness=-0.3:0.3",
    "E mesh.wireframe; thickness=0:0.2; offset=0:1; use_boundary; use_even_offset; use_relative_offset; use_replace; use_crease",
    "E mesh.quads_convert_to_tris; quad_method; ngon_method",
    "E mesh.tris_convert_to_quads; face_threshold=0:3; shape_threshold=0:3; uvs; seam; sharp; materials",
    "E mesh.poke; offset=-0.5:0.5; use_relative_offset; center_mode",
    "E mesh.vertices_smooth; factor=0:1; repeat=1:3; xaxis; yaxis; zaxis",
    "E mesh.symmetrize; direction; threshold=0:0.1",
    "E mesh.decimate; ratio=0.2:1; use_symmetry; symmetry_axis",
    "E mesh.normals_tools; !mode=RESET | ADD | MULTIPLY",
    "E mesh.smooth_normals; factor=0:1",
    "E mesh.average_normals; average_type; weight=1:100; threshold=0:5",
    "E mesh.merge_normals",
    "E mesh.split_normals",
    "E mesh.sort_elements; type=VIEW_ZAXIS | CURSOR_DISTANCE | MATERIAL | SELECTED | RANDOMIZE | REVERSE; elements; reverse; seed=0:100",
    "E mesh.reorder_vertices_spatial",
    "E mesh.symmetry_snap; direction; threshold=0:0.5; factor=0:1; use_center",
    "E mesh.face_make_planar; factor=0:1; repeat=1:4",
    "E mesh.flatten; factor=0:1",
    "E mesh.uvs_rotate; use_ccw",
    "E mesh.uvs_reverse",
    "A mesh.uv_texture_add",
    "A mesh.uv_texture_remove",
    "A mesh.customdata_custom_splitnormals_add",
    "A mesh.customdata_custom_splitnormals_clear",
    // --- transform ---------------------------------------------------------------------------
    "A transform.translate; !value=-1:1; constraint_axis; orient_type=GLOBAL | LOCAL | NORMAL; use_proportional_edit; proportional_size=0.5:2; proportional_edit_falloff=SMOOTH | SPHERE | ROOT | INVERSE_SQUARE | SHARP | LINEAR | CONSTANT",
    "A transform.rotate; !value=-3:3; !orient_axis; orient_type=GLOBAL | LOCAL; use_proportional_edit; proportional_size=0.5:2",
    "A transform.resize; !value=0.2:2; constraint_axis; orient_type=GLOBAL | LOCAL; use_proportional_edit; proportional_size=0.5:2",
    "E transform.mirror; !constraint_axis; orient_type=GLOBAL | LOCAL",
    "A transform.shear; !angle=-1:1; orient_axis; orient_axis_ortho",
    "E transform.shrink_fatten; !value=-0.5:0.5; use_even_offset",
    "E transform.tosphere; !value=0:1",
    "E transform.push_pull; !value=-1:1",
    "E transform.vertex_random; offset=0:0.3; uniform=0:1; normal=0:1; !seed=0:100",
    "E transform.edge_crease; !value=-1:1",
    "E transform.edge_bevelweight; !value=-1:1",
    "E transform.vert_crease; !value=-1:1",
    // --- view3d: cursor and snapping ---------------------------------------------------------------
    "O view3d.snap_cursor_to_center",
    "O view3d.snap_cursor_to_selected",
    "O view3d.snap_cursor_to_active",
    "O view3d.snap_cursor_to_grid",
    "O view3d.snap_selected_to_cursor; use_offset",
    "O view3d.snap_selected_to_active",
    "O view3d.snap_selected_to_grid",
    // --- uv ----------------------------------------------------------------------------------------
    "E uv.unwrap; method=ANGLE_BASED | CONFORMAL; fill_holes; correct_aspect; margin=0:0.1",
    "E uv.smart_project; angle_limit=0.3:1.4; island_margin=0:0.1; area_weight=0:1; correct_aspect; scale_to_bounds",
    "E uv.cube_project; cube_size=0.5:3; correct_aspect; clip_to_bounds; scale_to_bounds",
    "E uv.cylinder_project; direction=ALIGN_TO_OBJECT; align; radius=0.5:2; correct_aspect; clip_to_bounds; scale_to_bounds",
    "E uv.sphere_project; direction=ALIGN_TO_OBJECT; align; correct_aspect; clip_to_bounds; scale_to_bounds",
    "E uv.reset",
    "E uv.average_islands_scale; scale_uv; shear",
    "E uv.seams_from_islands; mark_seams; mark_sharp",
    "E uv.mark_seam; clear",
    "E uv.lightmap_pack; PREF_CONTEXT=SEL_FACES | ALL_FACES; PREF_PACK_IN_ONE; PREF_MARGIN_DIV=0.1:1",
    "E uv.pin; clear; invert",
    "E uv.select_all; !action",
    "E uv.select_linked",
    "E uv.select_split",
    "E uv.select_overlap; extend",
    "E uv.select_pinned",
    "E uv.hide; unselected",
    "E uv.reveal; select",
    "E uv.weld",
    "E uv.remove_doubles; threshold=0:0.1; use_unselected",
    "E uv.align; axis",
    "E uv.randomize_uv_transform; random_seed=0:100; use_loc; loc=-0.2:0.2; use_rot; rot=-1:1; use_scale; scale_even",
    // --- object: modes, selection, adding ------------------------------------------------------
    "O object.select_all; !action",
    "O object.select_by_type; extend; !type",
    "O object.select_pattern; !pattern='Cu*' | '*' | 'Light' | 'Cube*'; case_sensitive; extend",
    "O object.select_hierarchy; !direction; extend",
    "O object.select_random; ratio=0:1; !seed=0:1000; action",
    "O object.select_camera; extend",
    "O object.select_mirror; extend",
    "O object.select_more",
    "O object.select_less",
    "O object.camera_add; location=-3:3; rotation=-1:1",
    "O object.light_add; type; location=-3:3",
    "O object.empty_add; type; radius=0.5:2; location=-3:3",
    "O object.add; type=MESH | EMPTY; location=-3:3",
    "O object.delete; use_global",
    "O object.duplicate; linked",
    "O object.duplicate_move; !TRANSFORM_OT_translate={'value': (1, 0, 0)} | {'value': (0, 2, 0.5)}",
    "O object.duplicate_move_linked; !TRANSFORM_OT_translate={'value': (0, 1, 0)}",
    "O object.join",
    "O object.convert; !target=MESH; keep_original",
    "O object.parent_set; type=OBJECT; keep_transform",
    "O object.parent_clear; type",
    "O object.parent_no_inverse_set; keep_transform",
    "O object.parent_inverse_apply",
    "O object.origin_set; !type; center",
    "O object.transform_apply; location; rotation; scale; properties",
    "O object.location_clear; clear_delta",
    "O object.rotation_clear; clear_delta",
    "O object.scale_clear; clear_delta",
    "O object.origin_clear",
    "O object.visual_transform_apply",
    "O object.transforms_to_deltas; mode; reset_values",
    "O object.randomize_transform; !random_seed=0:100; use_delta; use_loc; loc=0:1; use_rot; rot=0:1; use_scale; scale_even; scale=0.5:1.5",
    "O object.align; bb_quality; align_mode; relative_to; align_axis",
    "O object.shade_smooth; keep_sharp_edges",
    "O object.shade_flat; keep_sharp_edges",
    "O object.shade_smooth_by_angle; angle=0.1:3; keep_sharp_edges",
    // --- object: modifiers ----------------------------------------------------------------------
    "O object.modifier_add; !type=SUBSURF | MIRROR | ARRAY | BEVEL | SOLIDIFY | TRIANGULATE | DECIMATE | WELD | DISPLACE | EDGE_SPLIT | WEIGHTED_NORMAL | SMOOTH | CAST | SIMPLE_DEFORM | WIREFRAME | SCREW | BUILD | LAPLACIANSMOOTH",
    "O object.modifier_remove; !modifier=$mod",
    "O object.modifier_apply; !modifier=$mod",
    "O object.modifier_move_up; !modifier=$mod",
    "O object.modifier_move_down; !modifier=$mod",
    "O object.modifier_move_to_index; !modifier=$mod; !index=0:2",
    "O object.modifier_copy; !modifier=$mod",
    "O object.modifier_set_active; !modifier=$mod",
    "O object.modifiers_clear",
    "O object.subdivision_set; level=0:2; relative",
    // --- object: materials, vertex groups, shape keys -----------------------------------------------
    "A object.material_slot_add",
    "A object.material_slot_remove",
    "O object.material_slot_remove_unused",
    "O object.material_slot_remove_all",
    "E object.material_slot_assign",
    "E object.material_slot_select",
    "E object.material_slot_deselect",
    "A object.material_slot_move; direction",
    "A object.vertex_group_add",
    "A object.vertex_group_remove; all",
    "E object.vertex_group_assign",
    "E object.vertex_group_remove_from; use_all_groups; use_all_verts",
    "E object.vertex_group_assign_new",
    "E object.vertex_group_select",
    "E object.vertex_group_deselect",
    "A object.vertex_group_set_active; !group=$vg",
    "A object.vertex_group_normalize",
    "A object.vertex_group_normalize_all; lock_active",
    "A object.vertex_group_clean; limit=0:0.5; keep_single",
    "A object.vertex_group_limit_total; limit=1:3",
    "A object.vertex_group_copy",
    "A object.vertex_group_invert; auto_assign; auto_remove",
    "A object.vertex_group_quantize; steps=1:8",
    "A object.vertex_group_levels; offset=-0.5:0.5; gain=0:2",
    "A object.vertex_group_lock; !action; mask",
    "A object.vertex_group_move; !direction",
    "A object.vertex_group_sort; sort_type",
    "O object.shape_key_add; from_mix",
    "O object.shape_key_remove; all; apply_mix",
    "O object.shape_key_clear",
    "O object.shape_key_move; !type",
    "O object.shape_key_lock; !action",
    // --- object: collections, visibility, data -------------------------------------------------------
    "O object.collection_add",
    "O object.hide_view_set; unselected",
    "O object.hide_view_clear; select",
    "O object.make_single_user; type; object; obdata",
    "O object.make_links_data; !type=OBDATA | MATERIAL | MODIFIERS",
    // --- geometry (attributes) -----------------------------------------------------------------------
    "O geometry.attribute_add; !name='Attr' | 'weight'; domain; data_type=FLOAT | INT | FLOAT_VECTOR | BOOLEAN | FLOAT_COLOR",
    "O geometry.attribute_remove",
    "O geometry.color_attribute_add; !name='Col' | 'Paint'; domain=POINT | CORNER; data_type",
    "O geometry.color_attribute_remove",
    "O geometry.color_attribute_duplicate",
];

/// The RNA statement templates (see the module docs).
pub const RNA: &[&str] = &[
    "A bpy.context.object.location = $v3(-3,3)",
    "A bpy.context.object.location.z = $f(-2,2)",
    "A bpy.context.object.rotation_euler = $v3(-3,3)",
    "A bpy.context.object.scale = $v3(0.2,3)",
    "A bpy.context.object.rotation_mode = $c('XYZ'|'ZXY'|'QUATERNION'|'AXIS_ANGLE')",
    "A bpy.context.object.delta_location = $v3(-1,1)",
    "A bpy.context.object.name = $c('Renamed'|'Cube'|'Object')",
    "A bpy.context.scene.cursor.location = $v3(-2,2)",
    "A bpy.context.scene.cursor.rotation_euler = $v3(-3,3)",
    "A bpy.context.scene.tool_settings.transform_pivot_point = $c('MEDIAN_POINT'|'CURSOR'|'INDIVIDUAL_ORIGINS'|'ACTIVE_ELEMENT'|'BOUNDING_BOX_CENTER')",
    "E bpy.context.scene.tool_settings.mesh_select_mode = $c((True, False, False)|(False, True, False)|(False, False, True)|(True, True, False))",
    "O bpy.context.object.data.vertices[$i(0,7)].co = $v3(-2,2)",
    "O bpy.context.object.data.polygons[$i(0,5)].use_smooth = $b",
    "O bpy.context.object.data.vertices[$i(0,7)].select = $b",
    "O bpy.context.object.data.polygons[$i(0,5)].select = $b",
    "O bpy.context.object.data.edges[$i(0,11)].use_seam = $b",
    "O bpy.context.object.data.name = $c('MeshData'|'Cube')",
    "A bpy.context.object.vertex_groups.new(name=$c('Group'|'Weights'))",
    "O bpy.context.object.data.uv_layers.new(name=$c('UVMap'|'UV2'))",
    "O bpy.context.object.shape_key_add(name=$c('Key'|'Basis'), from_mix=$b)",
    "A bpy.data.objects[$obj].select_set($b)",
    "O bpy.context.view_layer.objects.active = bpy.data.objects[$obj]",
    "A bpy.data.materials.new($c('Mat'|'Red'))",
    "O bpy.context.object.data.materials.append(bpy.data.materials[$c('Mat'|'Material')])",
    "O bpy.context.object.modifiers.new($c('Sub'|'Arr'), $c('SUBSURF'|'ARRAY'|'MIRROR'|'BEVEL'))",
    "O bpy.context.object.modifiers[$mod].show_viewport = $b",
    "O bpy.context.object.modifiers['Subdivision'].levels = $i(0,2)",
    "O bpy.context.object.modifiers['Subdivision'].subdivision_type = $c('CATMULL_CLARK'|'SIMPLE')",
    "O bpy.context.object.modifiers['Array'].count = $i(1,4)",
    "O bpy.context.object.modifiers['Array'].relative_offset_displace = $v3(-1,1.5)",
    "O bpy.context.object.modifiers['Mirror'].use_axis = $b3",
    "O bpy.context.object.modifiers['Mirror'].use_bisect_axis = $b3",
    "O bpy.context.object.modifiers['Bevel'].width = $f(0,0.4)",
    "O bpy.context.object.modifiers['Bevel'].segments = $i(1,4)",
    "O bpy.context.object.modifiers['Solidify'].thickness = $f(-0.4,0.4)",
    "O bpy.context.object.modifiers['Decimate'].ratio = $f(0.2,1)",
    "O bpy.context.object.modifiers['Weld'].merge_threshold = $f(0,0.5)",
    "O bpy.context.object.modifiers['Displace'].strength = $f(-1,1)",
    "O bpy.context.object.modifiers['Smooth'].factor = $f(-1,1)",
    "O bpy.context.object.modifiers['Cast'].factor = $f(-1,1)",
    "O bpy.context.object.modifiers['SimpleDeform'].deform_method = $c('TWIST'|'BEND'|'TAPER'|'STRETCH')",
    "O bpy.context.object.modifiers['Wireframe'].thickness = $f(0,0.2)",
    "O bpy.context.object.modifiers['Screw'].steps = $i(3,12)",
    "O bpy.context.object.modifiers['Triangulate'].quad_method = $c('BEAUTY'|'FIXED'|'FIXED_ALTERNATE'|'SHORTEST_DIAGONAL'|'LONGEST_DIAGONAL')",
    "O bpy.context.object.modifiers['EdgeSplit'].split_angle = $f(0,3)",
];

/// Spec files the catalogue reads (Blender 5.2.4 operator dumps).
const SPEC: &[(&str, &str)] = &[
    (
        "mesh",
        include_str!("../../../../spec/blender-5.2/ops/mesh.json"),
    ),
    (
        "object",
        include_str!("../../../../spec/blender-5.2/ops/object.json"),
    ),
    (
        "transform",
        include_str!("../../../../spec/blender-5.2/ops/transform.json"),
    ),
    (
        "view3d",
        include_str!("../../../../spec/blender-5.2/ops/view3d.json"),
    ),
    (
        "uv",
        include_str!("../../../../spec/blender-5.2/ops/uv.json"),
    ),
    (
        "geometry",
        include_str!("../../../../spec/blender-5.2/ops/geometry.json"),
    ),
];

/// Every operator definition of [`SPEC`], by idname.
pub fn spec_ops() -> &'static BTreeMap<String, Json> {
    static OPS: OnceLock<BTreeMap<String, Json>> = OnceLock::new();
    OPS.get_or_init(|| {
        let mut out = BTreeMap::new();
        for (ns, text) in SPEC {
            let v: Json = serde_json::from_str(text)
                .unwrap_or_else(|e| panic!("invariant: spec/blender-5.2/ops/{ns}.json: {e}"));
            for op in v["operators"].as_array().into_iter().flatten() {
                if let Some(id) = op["idname"].as_str() {
                    out.insert(id.to_owned(), op.clone());
                }
            }
        }
        out
    })
}

/// A catalogue error (bad line, unknown operator or property).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogError(pub String);

impl std::fmt::Display for CatalogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

fn num(v: &Json) -> Option<f64> {
    v.as_f64()
}

/// The property's [`Kind`] from its spec definition and an optional override.
fn kind_of(op: &str, prop: &Json, over: Option<&str>) -> Result<Kind, CatalogError> {
    let name = prop["identifier"].as_str().unwrap_or("?");
    let err = |m: String| CatalogError(format!("{op}.{name}: {m}"));
    let ty = prop["type"].as_str().unwrap_or("");
    let n = prop["array"].as_array().map_or(0, |a| {
        a.iter().filter_map(Json::as_u64).product::<u64>() as usize
    });
    if let Some(o) = over {
        if o.contains(" | ") || !o.contains(':') || o.starts_with('{') || o.starts_with('(') {
            return Ok(Kind::Literal(
                o.split(" | ")
                    .map(|s| {
                        let s = s.trim();
                        // Bare enum identifiers become string literals.
                        if ty == "ENUM"
                            && s.chars()
                                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
                        {
                            format!("'{s}'")
                        } else {
                            s.to_owned()
                        }
                    })
                    .collect(),
            ));
        }
        let (lo, hi) = o
            .split_once(':')
            .ok_or_else(|| err(format!("bad override `{o}`")))?;
        let (lo, hi): (f64, f64) = (
            lo.parse().map_err(|_| err(format!("bad bound `{lo}`")))?,
            hi.parse().map_err(|_| err(format!("bad bound `{hi}`")))?,
        );
        return match ty {
            "INT" => Ok(Kind::Int {
                lo: lo as i64,
                hi: hi as i64,
                n,
            }),
            "FLOAT" => Ok(Kind::Float { lo, hi, n }),
            _ => Err(err(format!("a range for a {ty} property"))),
        };
    }
    let soft = |k: &str, d: f64| num(&prop[k]).unwrap_or(d);
    Ok(match ty {
        "BOOLEAN" => Kind::Bool { n },
        "INT" => Kind::Int {
            lo: soft("soft_min", 0.0).max(-3.0) as i64,
            hi: soft("soft_max", 5.0).min(5.0) as i64,
            n,
        },
        "FLOAT" => Kind::Float {
            lo: soft("soft_min", -1.0).max(-2.0),
            hi: soft("soft_max", 1.0).min(2.0),
            n,
        },
        "ENUM" => {
            let items: Vec<String> = prop["items"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|i| i["identifier"].as_str().map(str::to_owned))
                .collect();
            if items.is_empty() {
                return Err(err("an enum without static items needs an override".into()));
            }
            let flag = prop["flags"]
                .as_array()
                .is_some_and(|f| f.iter().any(|x| x == "ENUM_FLAG"));
            if flag {
                Kind::EnumFlag(items)
            } else {
                Kind::Enum(items)
            }
        }
        _ => return Err(err(format!("type {ty} needs an override"))),
    })
}

/// Parses one operator line against the spec.
pub fn parse_op(line: &str) -> Result<OpEntry, CatalogError> {
    let err = |m: String| CatalogError(format!("`{line}`: {m}"));
    let mut parts = line.split(';').map(str::trim);
    let head = parts.next().unwrap_or("");
    let (ctx, idname) = head
        .split_once(' ')
        .ok_or_else(|| err("expected `<ctx> <idname>`".into()))?;
    let ctx = Ctx::parse(ctx).ok_or_else(|| err(format!("unknown context `{ctx}`")))?;
    let spec = spec_ops()
        .get(idname)
        .ok_or_else(|| err(format!("`{idname}` is not a Blender 5.2.4 operator")))?;
    let mut props = Vec::new();
    for p in parts.filter(|p| !p.is_empty()) {
        let (always, p) = match p.strip_prefix('!') {
            Some(rest) => (true, rest),
            None => (false, p),
        };
        let (name, over) = match p.split_once('=') {
            Some((n, o)) => (n.trim(), Some(o.trim())),
            None => (p, None),
        };
        let def = spec["props"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|d| d["identifier"] == name)
            .ok_or_else(|| err(format!("`{idname}` has no property `{name}`")))?;
        // Macro steps (`TRANSFORM_OT_translate`) are POINTER properties: literals only.
        let kind = if def["type"] == "POINTER" {
            match over {
                Some(o) => Kind::Literal(o.split(" | ").map(|s| s.trim().to_owned()).collect()),
                None => return Err(err(format!("macro step `{name}` needs literals"))),
            }
        } else {
            kind_of(idname, def, over)?
        };
        props.push(PropGen {
            name: name.to_owned(),
            always,
            kind,
        });
    }
    Ok(OpEntry {
        idname: idname.to_owned(),
        ctx,
        props,
    })
}

/// Parses one RNA template line.
pub fn parse_rna(line: &str) -> Result<RnaEntry, CatalogError> {
    let (ctx, template) = line
        .split_once(' ')
        .ok_or_else(|| CatalogError(format!("`{line}`: expected `<ctx> <python>`")))?;
    Ok(RnaEntry {
        ctx: Ctx::parse(ctx).ok_or_else(|| CatalogError(format!("`{line}`: bad context")))?,
        template: template.to_owned(),
    })
}

/// The parsed catalogue.
#[derive(Clone, Debug)]
pub struct Catalog {
    /// Operators.
    pub ops: Vec<OpEntry>,
    /// RNA statements.
    pub rna: Vec<RnaEntry>,
}

impl Catalog {
    /// [`OPERATORS`] and [`RNA`], parsed (once).
    pub fn get() -> &'static Catalog {
        static C: OnceLock<Catalog> = OnceLock::new();
        C.get_or_init(|| {
            Catalog::parse(OPERATORS, RNA).unwrap_or_else(|e| panic!("invariant: catalogue: {e}"))
        })
    }

    /// Parses catalogue lines.
    pub fn parse(ops: &[&str], rna: &[&str]) -> Result<Catalog, CatalogError> {
        Ok(Catalog {
            ops: ops.iter().map(|l| parse_op(l)).collect::<Result<_, _>>()?,
            rna: rna.iter().map(|l| parse_rna(l)).collect::<Result<_, _>>()?,
        })
    }

    /// Distinct operator idnames.
    pub fn operator_ids(&self) -> Vec<&str> {
        let mut v: Vec<&str> = self.ops.iter().map(|o| o.idname.as_str()).collect();
        v.sort_unstable();
        v.dedup();
        v
    }
}
