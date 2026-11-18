# frap operator and RNA checklist

Companion to `docs/ARCHITECTURE.md`. Every operator, RNA struct, function, modifier type and shader node in scope, with the milestone it lands in. Operator names were checked against the operator list of Blender 5.2.4 (`-b --factory-startup`) when this file was generated.

**Legend.** `M1`…`M8`: milestone (ARCHITECTURE §22). `OUT`: not planned; Blender has it, frap answers `NotImplementedError` (`OP_NOT_IMPLEMENTED`) and `--describe` lists it as `not_implemented`. Priority `P0` MVP, `P1` needed for the fixed scope, `P2` parity/nice to have (from the research catalogues). *macro*: takes nested `<SUB>_OT_<op>={...}` props. *headless-extended*: frap does more than `blender -b` (documented in `docs/deviations.md`; goldens use `#@blender` emulations). Every implemented operator must also reach fidelity tier T0; target tiers above T0 are noted where decided. *prereq*: part of a milestone's golden prerequisite set (ARCHITECTURE §22.1), owned by a WP one wave earlier than the goldens that use it.

The `M` column is machine-read: `cargo xtask codegen` generates typed props structs (`frap-spec-ops-<group>`) for every operator with `M` ≤ `spec/current_milestone`; all others are served from `frap-spec-catalog` (ARCHITECTURE §4.2). Keep the column format (`M<n>` or `OUT`) exact.

Rules: (1) an operator assigned to a milestone but not named in a WP file list of ARCHITECTURE §22 is assigned by that milestone's Wave 0 integrator to the WP owning the closest area; (2) tick the box when the operator is registered, its golden cases pass at the declared tier, and its `--describe` snapshot exists; (3) every operator of the curated namespaces that is not listed here is `OUT`: UI/editor-only operators and out-of-scope features (grease pencil, particles, physics, hooks, multires, light linking, geometry-node baking, constraints, drivers, other file formats); e.g. the 101 unlisted `object.*` operators are those marked OUT in [OBJ §6].

## A. Operators

### `mesh.* (primitives and Edit Mode)`

| ✓ | Operator | M | Prio | Notes |
|---|---|---|---|---|
| [x] | `mesh.primitive_plane_add` | M1 | P0 | T1 vs dumps; edit-mode merge in M2 |
| [x] | `mesh.primitive_cube_add` | M1 | P0 | worked example (ARCHITECTURE §10.4) |
| [x] | `mesh.primitive_circle_add` | M1 | P0 |  |
| [x] | `mesh.primitive_uv_sphere_add` | M1 | P0 |  |
| [x] | `mesh.primitive_ico_sphere_add` | M1 | P0 |  |
| [x] | `mesh.primitive_cylinder_add` | M1 | P0 |  |
| [x] | `mesh.primitive_cone_add` | M1 | P0 |  |
| [x] | `mesh.primitive_torus_add` | M1 | P0 |  |
| [x] | `mesh.primitive_grid_add` | M1 | P0 |  |
| [x] | `mesh.primitive_monkey_add` | M1 | P0 | depends on licence decision D1 |
| [x] | `mesh.select_all` | M2 | P0 | prereq (WP 2.2) |
| [x] | `mesh.select_mode` | M2 | P0 | prereq (WP 2.2); conversion table [MESH §3.3] |
| [x] | `mesh.select_linked` | M2 | P0 | default delimit SEAM; no last-delimit memory (D-64) |
| [x] | `mesh.select_more` | M2 | P0 |  |
| [x] | `mesh.select_less` | M2 | P0 |  |
| [x] | `mesh.select_edge_loop_multi` | M2 | P0 | replaces loop_multi_select |
| [x] | `mesh.select_edge_ring_multi` | M2 | P0 |  |
| [x] | `mesh.select_non_manifold` | M2 | P1 | VERT/EDGE mode only |
| [x] | `mesh.select_loose` | M2 | P1 |  |
| [x] | `mesh.select_face_by_sides` | M2 | P1 |  |
| [x] | `mesh.select_by_attribute` | M2 | P1 | active attribute = `mesh.attributes.active` (WP 2.9) |
| [x] | `mesh.loop_select` | M2 | P1 | headless-extended: edge_index |
| [x] | `mesh.edgering_select` | M2 | P1 | headless-extended: edge_index |
| [x] | `mesh.edges_select_sharp` | M2 | P1 |  |
| [x] | `mesh.hide` | M2 | P1 |  |
| [x] | `mesh.reveal` | M2 | P1 |  |
| [x] | `mesh.select_random` | M2 | P2 | BliRng, equal per seed |
| [x] | `mesh.faces_select_linked_flat` | M2 | P2 |  |
| [x] | `mesh.loop_to_region` | M2 | P2 |  |
| [x] | `mesh.region_to_loop` | M2 | P2 |  |
| [x] | `mesh.shortest_path_select` | M2 | P2 |  |
| [ ] | `mesh.select_nth` | M8 | P2 | undefined without active element |
| [ ] | `mesh.select_boundary_loop_multi` | M8 | P2 |  |
| [ ] | `mesh.select_similar` | M8 | P2 |  |
| [ ] | `mesh.select_similar_region` | M8 | P2 |  |
| [ ] | `mesh.select_interior_faces` | M8 | P2 |  |
| [ ] | `mesh.select_mirror` | M8 | P2 |  |
| [ ] | `mesh.select_axis` | M8 | P2 |  |
| [ ] | `mesh.select_ungrouped` | M8 | P2 | needs vertex groups (M6) |
| [ ] | `mesh.select_by_pole_count` | M8 | P2 |  |
| [ ] | `mesh.select_next_item` | M8 | P2 |  |
| [ ] | `mesh.select_prev_item` | M8 | P2 |  |
| [ ] | `mesh.select_linked_pick` | M8 | P2 | headless-extended: index |
| [ ] | `mesh.shortest_path_pick` | M8 | P2 | headless-extended: index |
| [x] | `mesh.extrude_region` | M2 | P0 | T1 |
| [x] | `mesh.extrude_region_move` | M2 | P0 | macro; T1 |
| [x] | `mesh.extrude_region_shrink_fatten` | M2 | P0 | macro |
| [x] | `mesh.extrude_context` | M2 | P0 |  |
| [x] | `mesh.extrude_context_move` | M2 | P0 | macro |
| [x] | `mesh.extrude_faces_indiv` | M2 | P1 |  |
| [x] | `mesh.extrude_faces_move` | M2 | P1 | macro |
| [x] | `mesh.extrude_edges_indiv` | M2 | P1 |  |
| [x] | `mesh.extrude_edges_move` | M2 | P1 | macro |
| [x] | `mesh.extrude_verts_indiv` | M2 | P1 |  |
| [x] | `mesh.extrude_vertices_move` | M2 | P1 | macro |
| [x] | `mesh.extrude_repeat` | M2 | P2 | offset is a vector in 5.x |
| [ ] | `mesh.extrude_manifold` | M2 | P2 | macro; partial: the translate step's `use_automerge_and_split` is not implemented in Edit Mode (OP_NOT_IMPLEMENTED with a hint: `mesh.extrude_region_move` + `mesh.remove_doubles`), golden `macros/manifold.py.pending` |
| [x] | `mesh.inset` | M2 | P0 | T1 |
| [x] | `mesh.bevel` | M2 | P0 | seg 1 T1, N segments T2; offset=0 no-op hint (NO_OP_DEFAULT). Partial: `profile_type='CUSTOM'` OP_NOT_IMPLEMENTED; harden_normals and Auto Merge not applied (WARNING), D-80 |
| [x] | `mesh.subdivide` | M2 | P0 | uniform T1 |
| [x] | `mesh.loopcut` | M2 | P0 | headless-extended: object_index/edge_index; Blender -b runs it under the golden runner's VIEW_3D override, so goldens call it directly (WP 2.6) |
| [x] | `mesh.loopcut_slide` | M2 | P0 | macro; headless-extended (edge_slide step: WP 2.12) |
| [x] | `mesh.subdivide_edgering` | M2 | P2 |  |
| [x] | `mesh.unsubdivide` | M2 | P2 |  |
| [x] | `mesh.offset_edge_loops` | M2 | P2 |  |
| [x] | `mesh.offset_edge_loops_slide` | M2 | P2 | macro |
| [x] | `mesh.merge` | M2 | P0 | dynamic enum FIRST/LAST (WP 2.7, T1) |
| [x] | `mesh.remove_doubles` | M2 | P0 | "Removed N vertex/vertices", `result.removed` (WP 2.7, T1) |
| [x] | `mesh.delete` | M2 | P0 | type is HIDDEN but essential (WP 2.7, T1) |
| [x] | `mesh.dissolve_verts` | M2 | P1 | WP 2.7, T1 |
| [x] | `mesh.dissolve_edges` | M2 | P1 | WP 2.7, T1 |
| [x] | `mesh.dissolve_faces` | M2 | P1 | WP 2.7, T1 |
| [x] | `mesh.dissolve_mode` | M2 | P1 | WP 2.7, T1 |
| [x] | `mesh.dissolve_limited` | M2 | P1 | WP 2.7: T1 (BLI_heap port) |
| [x] | `mesh.delete_loose` | M2 | P2 | WP 2.7, T1 |
| [x] | `mesh.delete_edgeloop` | M2 | P2 | WP 2.7, T1 |
| [x] | `mesh.dissolve_degenerate` | M2 | P2 | WP 2.7, T1 |
| [x] | `mesh.edge_collapse` | M2 | P2 | WP 2.7, T1 |
| [x] | `mesh.edge_face_add` | M2 | P0 | contextual create ("F") |
| [x] | `mesh.bridge_edge_loops` | M2 | P1 |  |
| [x] | `mesh.fill` | M2 | P1 |  |
| [x] | `mesh.fill_grid` | M2 | P1 |  |
| [x] | `mesh.vert_connect` | M2 | P1 |  |
| [x] | `mesh.vert_connect_path` | M2 | P1 |  |
| [x] | `mesh.fill_holes` | M2 | P2 |  |
| [x] | `mesh.beautify_fill` | M2 | P2 |  |
| [x] | `mesh.vert_connect_concave` | M2 | P2 |  |
| [x] | `mesh.vert_connect_nonplanar` | M2 | P2 |  |
| [x] | `mesh.face_split_by_edges` | M2 | P2 |  |
| [x] | `mesh.edge_rotate` | M2 | P2 |  |
| [x] | `mesh.bisect` | M2 | P2 | plane must be explicit headless |
| [x] | `mesh.convex_hull` | M2 | P2 |  |
| [x] | `mesh.flip_normals` | M2 | P0 | keeps l_first |
| [x] | `mesh.normals_make_consistent` | M2 | P0 |  |
| [x] | `mesh.faces_shade_smooth` | M2 | P0 |  |
| [x] | `mesh.faces_shade_flat` | M2 | P0 |  |
| [x] | `mesh.mark_sharp` | M2 | P0 |  |
| [x] | `mesh.mark_seam` | M2 | P0 | attribute uv_seam |
| [x] | `mesh.set_normals_from_faces` | M2 | P1 | writes custom_normal |
| [x] | `mesh.set_sharpness_by_angle` | M2 | P1 | also adds custom_normal (as Blender) |
| [x] | `mesh.customdata_custom_splitnormals_add` | M2 | P1 |  |
| [x] | `mesh.customdata_custom_splitnormals_clear` | M2 | P1 |  |
| [x] | `mesh.attribute_set` | M2 | P1 |  |
| [x] | `mesh.uv_texture_add` | M2 | P0 | copies active UV map |
| [x] | `mesh.uv_texture_remove` | M2 | P0 |  |
| [x] | `mesh.split` | M2 | P1 |  |
| [x] | `mesh.edge_split` | M2 | P1 |  |
| [x] | `mesh.separate` | M2 | P0 | partial: shape keys not carried (M6) |
| [x] | `mesh.duplicate` | M2 | P1 |  |
| [x] | `mesh.duplicate_move` | M2 | P0 | macro |
| [x] | `mesh.spin` | M2 | P1 | axis required headless |
| [x] | `mesh.screw` | M2 | P2 |  |
| [x] | `mesh.solidify` | M2 | P2 | edit-mode op (not the modifier) |
| [x] | `mesh.wireframe` | M2 | P2 |  |
| [x] | `mesh.quads_convert_to_tris` | M2 | P1 | FIXED T1, BEAUTY T2 |
| [x] | `mesh.tris_convert_to_quads` | M2 | P1 | T2 |
| [x] | `mesh.poke` | M2 | P2 | T1 |
| [x] | `mesh.flip_quad_tessellation` | M2 | P2 |  |
| [x] | `mesh.vertices_smooth` | M2 | P1 | factor=0 default is a no-op |
| [x] | `mesh.symmetrize` | M2 | P1 | partial: `use_mesh_automerge` not applied afterwards |
| [ ] | `mesh.decimate` | M4 | P2 | shares DECIMATE COLLAPSE |
| [ ] | `mesh.intersect` | M4 | P2 | shares Boolean solver |
| [ ] | `mesh.intersect_boolean` | M4 | P2 | solver FLOAT/EXACT (5.x renamed FAST) |
| [ ] | `mesh.blend_from_shape` | M6 | P2 | shape keys |
| [ ] | `mesh.shape_propagate_to_all` | M6 | P2 | shape keys |
| [ ] | `mesh.normals_tools` | M8 | P2 |  |
| [ ] | `mesh.smooth_normals` | M8 | P2 |  |
| [ ] | `mesh.average_normals` | M8 | P2 |  |
| [ ] | `mesh.point_normals` | M8 | P2 |  |
| [ ] | `mesh.merge_normals` | M8 | P2 |  |
| [ ] | `mesh.split_normals` | M8 | P2 |  |
| [ ] | `mesh.mod_weighted_strength` | M8 | P2 |  |
| [ ] | `mesh.colors_rotate` | M8 | P2 |  |
| [ ] | `mesh.colors_reverse` | M8 | P2 |  |
| [ ] | `mesh.uvs_rotate` | M8 | P2 |  |
| [ ] | `mesh.uvs_reverse` | M8 | P2 |  |
| [ ] | `mesh.sort_elements` | M8 | P2 |  |
| [ ] | `mesh.reorder_vertices_spatial` | M8 | P2 |  |
| [ ] | `mesh.rip` | M8 | P2 | headless-extended: explicit direction |
| [ ] | `mesh.rip_move` | M8 | P2 | macro |
| [ ] | `mesh.rip_edge` | M8 | P2 |  |
| [ ] | `mesh.rip_edge_move` | M8 | P2 | macro |
| [ ] | `mesh.vertices_smooth_laplacian` | M8 | P2 |  |
| [ ] | `mesh.symmetry_snap` | M8 | P2 |  |
| [ ] | `mesh.face_make_planar` | M8 | P2 |  |
| [ ] | `mesh.circularize` | M8 | P2 | new in 5.x |
| [ ] | `mesh.flatten` | M8 | P2 | new in 5.x |
| [ ] | `mesh.space_edge_loops_evenly` | M8 | P2 | new in 5.x |
| — | `mesh.knife_project` | OUT | — | view-dependent |
| — | `mesh.knife_tool` | OUT | — | interactive |
| — | `mesh.dupli_extrude_cursor` | OUT | — | interactive |
| — | `mesh.primitive_cube_add_gizmo` | OUT | — | interactive |
| — | `mesh.mark_freestyle_edge` | OUT | — | Freestyle |
| — | `mesh.mark_freestyle_face` | OUT | — | Freestyle |
| — | `mesh.customdata_face_sets_clear` | OUT | — | sculpt |
| — | `mesh.customdata_mask_clear` | OUT | — | sculpt |
| — | `mesh.customdata_skin_add` | OUT | — | Skin modifier |
| — | `mesh.customdata_skin_clear` | OUT | — | Skin modifier |
| — | `mesh.polybuild_*` | OUT | — | interactive (9 operators) |

### `geometry.* (attributes)`

| ✓ | Operator | M | Prio | Notes |
|---|---|---|---|---|
| [x] | `geometry.color_attribute_add` | M2 | P0 | name '' -> "Attribute", FLOAT_COLOR POINT; glTF COLOR_0 |
| [x] | `geometry.color_attribute_remove` | M2 | P1 |  |
| [x] | `geometry.color_attribute_render_set` | M2 | P1 |  |
| [x] | `geometry.attribute_add` | M2 | P1 |  |
| [x] | `geometry.attribute_remove` | M2 | P1 |  |
| [x] | `geometry.attribute_convert` | M2 | P1 |  |
| [x] | `geometry.color_attribute_duplicate` | M2 | P2 |  |
| [x] | `geometry.color_attribute_convert` | M2 | P2 |  |
| — | `geometry.geometry_randomization` | OUT | — | debug |

### `object.*`

| ✓ | Operator | M | Prio | Notes |
|---|---|---|---|---|
| [x] | `object.mode_set` | M1 | P0 | prereq (WP 1.19); dispatcher + `modes/object.rs` M1; `modes/edit_mesh.rs` M2 (WP 2.2); `modes/{edit_armature,pose}.rs` M6 (WP 6.2) |
| [x] | `object.editmode_toggle` | M2 | P1 |  |
| [ ] | `object.posemode_toggle` | M6 | P1 |  |
| [ ] | `object.mode_set_with_submode` | M8 | P2 |  |
| [x] | `object.select_all` | M1 | P0 | prereq (WP 1.19) |
| [x] | `object.select_by_type` | M1 | P1 |  |
| [x] | `object.select_pattern` | M1 | P1 |  |
| [ ] | `object.select_grouped` | M8 | P2 |  |
| [ ] | `object.select_hierarchy` | M8 | P2 |  |
| [ ] | `object.select_linked` | M8 | P2 |  |
| [ ] | `object.select_random` | M8 | P2 |  |
| [ ] | `object.select_camera` | M8 | P2 |  |
| [ ] | `object.select_mirror` | M8 | P2 |  |
| [ ] | `object.select_more` | M8 | P2 |  |
| [ ] | `object.select_less` | M8 | P2 |  |
| [ ] | `object.select_same_collection` | M8 | P2 |  |
| [x] | `object.camera_add` | M1 | P0 | sets scene.camera when None (virtual 3D View); object and data uniquified independently |
| [x] | `object.light_add` | M1 | P0 | 10 W (factory Light is 1000 W) |
| [x] | `object.empty_add` | M1 | P0 |  |
| [ ] | `object.armature_add` | M6 | P0 |  |
| [ ] | `object.add` | M8 | P2 |  |
| [ ] | `object.add_named` | M8 | P2 |  |
| [ ] | `object.collection_instance_add` | M8 | P2 |  |
| [x] | `object.delete` | M1 | P0 | prereq (WP 1.19); leaves orphan data; re-adding then names data `.001` (T0) |
| [x] | `object.duplicate` | M1 | P0 | materials shared (factory prefs) |
| [x] | `object.duplicate_move` | M1 | P0 | macro |
| [x] | `object.duplicate_move_linked` | M1 | P1 | macro |
| [x] | `object.join` | M1 | P0 | CANCELLED with WARNING reports |
| [ ] | `object.convert` | M4 | P1 | apply modifiers to mesh |
| [ ] | `object.join_shapes` | M6 | P2 |  |
| [ ] | `object.duplicates_make_real` | M8 | P2 |  |
| [x] | `object.parent_set` | M1 | P0 | OBJECT/KEEP types M1; ARMATURE_* M6 |
| [x] | `object.parent_clear` | M1 | P0 |  |
| [x] | `object.parent_no_inverse_set` | M1 | P1 |  |
| [ ] | `object.parent_inverse_apply` | M8 | P2 |  |
| [x] | `object.origin_set` | M1 | P0 | all types |
| [x] | `object.transform_apply` | M1 | P0 | multi-user error as Blender |
| [x] | `object.location_clear` | M1 | P0 |  |
| [x] | `object.rotation_clear` | M1 | P0 |  |
| [x] | `object.scale_clear` | M1 | P0 |  |
| [ ] | `object.origin_clear` | M8 | P2 |  |
| [ ] | `object.visual_transform_apply` | M8 | P2 |  |
| [ ] | `object.transforms_to_deltas` | M8 | P2 |  |
| [ ] | `object.randomize_transform` | M8 | P2 | BliRng |
| [ ] | `object.align` | M8 | P2 |  |
| [x] | `object.shade_smooth` | M1 | P0 | removes sharp_face |
| [x] | `object.shade_flat` | M1 | P0 |  |
| [x] | `object.shade_smooth_by_angle` | M1 | P0 | destructive, writes sharp_edge |
| [ ] | `object.shade_auto_smooth` | M4 | P1 | adds "Smooth by Angle" NODES modifier; links the group from a `Library` (2 `bpy.data.libraries` entries, as Blender) |
| [ ] | `object.modifier_add` | M4 | P0 | dynamic enum by object type |
| [ ] | `object.modifier_remove` | M4 | P0 |  |
| [ ] | `object.modifier_apply` | M4 | P0 |  |
| [ ] | `object.modifier_move_up` | M4 | P0 |  |
| [ ] | `object.modifier_move_down` | M4 | P0 |  |
| [ ] | `object.modifier_move_to_index` | M4 | P0 |  |
| [ ] | `object.modifier_copy` | M4 | P1 |  |
| [ ] | `object.modifier_set_active` | M4 | P1 |  |
| [ ] | `object.modifiers_clear` | M4 | P1 |  |
| [ ] | `object.subdivision_set` | M4 | P1 |  |
| [ ] | `object.modifier_apply_as_shapekey` | M6 | P2 |  |
| [ ] | `object.modifier_copy_to_selected` | M8 | P2 |  |
| [ ] | `object.modifiers_copy_to_selected` | M8 | P2 |  |
| [x] | `object.material_slot_add` | M1 | P0 |  |
| [x] | `object.material_slot_remove` | M1 | P0 |  |
| [x] | `object.material_slot_remove_unused` | M1 | P1 |  |
| [x] | `object.material_slot_remove_all` | M1 | P2 |  |
| [x] | `object.material_slot_assign` | M2 | P0 | edit-mode face assignment (object mode: no-op as Blender); WP 2.9 (M2 W0 assignment) |
| [x] | `object.material_slot_select` | M2 | P1 | edit mode; WP 2.4 (M2 W0 assignment) |
| [x] | `object.material_slot_deselect` | M2 | P1 | edit mode; WP 2.4 (M2 W0 assignment) |
| [ ] | `object.material_slot_move` | M3 | P2 | WP 3.1 (M3+M4 W0 assignment) |
| [ ] | `object.material_slot_copy` | M8 | P2 |  |
| [ ] | `object.vertex_group_add` | M6 | P0 |  |
| [ ] | `object.vertex_group_remove` | M6 | P0 |  |
| [ ] | `object.vertex_group_assign` | M6 | P0 | edit mode |
| [ ] | `object.vertex_group_remove_from` | M6 | P0 | edit mode |
| [ ] | `object.vertex_group_normalize_all` | M6 | P0 |  |
| [ ] | `object.vertex_group_limit_total` | M6 | P0 | glTF 4-influence limit |
| [ ] | `object.vertex_group_assign_new` | M6 | P1 |  |
| [ ] | `object.vertex_group_select` | M6 | P1 |  |
| [ ] | `object.vertex_group_deselect` | M6 | P1 |  |
| [ ] | `object.vertex_group_set_active` | M6 | P1 | dynamic enum |
| [ ] | `object.vertex_group_normalize` | M6 | P1 |  |
| [ ] | `object.vertex_group_clean` | M6 | P1 |  |
| [ ] | `object.vertex_group_copy` | M8 | P2 |  |
| [ ] | `object.vertex_group_copy_to_selected` | M8 | P2 |  |
| [ ] | `object.vertex_group_invert` | M8 | P2 |  |
| [ ] | `object.vertex_group_quantize` | M8 | P2 |  |
| [ ] | `object.vertex_group_levels` | M8 | P2 |  |
| [ ] | `object.vertex_group_lock` | M8 | P2 |  |
| [ ] | `object.vertex_group_mirror` | M8 | P2 |  |
| [ ] | `object.vertex_group_move` | M8 | P2 |  |
| [ ] | `object.vertex_group_smooth` | M8 | P2 |  |
| [ ] | `object.vertex_group_sort` | M8 | P2 |  |
| [ ] | `object.vertex_weight_copy` | M8 | P2 |  |
| [ ] | `object.vertex_weight_delete` | M8 | P2 |  |
| [ ] | `object.vertex_weight_normalize_active_vertex` | M8 | P2 |  |
| [ ] | `object.vertex_weight_paste` | M8 | P2 |  |
| [ ] | `object.vertex_weight_set_active` | M8 | P2 |  |
| [ ] | `object.shape_key_add` | M6 | P0 |  |
| [ ] | `object.shape_key_remove` | M6 | P0 |  |
| [ ] | `object.shape_key_clear` | M6 | P1 |  |
| [ ] | `object.shape_key_move` | M6 | P1 |  |
| [ ] | `object.shape_key_mirror` | M8 | P2 |  |
| [ ] | `object.shape_key_copy` | M8 | P2 |  |
| [ ] | `object.shape_key_lock` | M8 | P2 |  |
| [ ] | `object.shape_key_make_basis` | M8 | P2 |  |
| [ ] | `object.shape_key_apply_to_basis` | M8 | P2 |  |
| [ ] | `object.shape_key_transfer` | M8 | P2 |  |
| [x] | `object.move_to_collection` | M1 | P1 |  |
| [x] | `object.link_to_collection` | M1 | P2 |  |
| [x] | `object.collection_add` | M1 | P2 |  |
| [x] | `object.collection_remove` | M1 | P2 |  |
| [x] | `object.collection_link` | M1 | P2 | dynamic enum |
| [x] | `object.collection_unlink` | M1 | P2 |  |
| [ ] | `object.hide_view_set` | M3 | P1 | virtual 3D View poll; WP 3.10 (M3+M4 W0 assignment) |
| [ ] | `object.hide_view_clear` | M3 | P1 | WP 3.10 (M3+M4 W0 assignment) |
| [ ] | `object.make_single_user` | M3 | P1 | WP 3.10 (M3+M4 W0 assignment) |
| [ ] | `object.make_links_data` | M3 | P1 | WP 3.10 (M3+M4 W0 assignment) |
| [ ] | `object.hide_render_clear_all` | M8 | P2 |  |
| [ ] | `object.track_set` | M8 | P2 |  |
| [ ] | `object.track_clear` | M8 | P2 |  |
| [ ] | `object.hide_collection` | M8 | P2 |  |
| [ ] | `object.collection_objects_select` | M8 | P2 |  |
| [ ] | `object.data_transfer` | M8 | P2 |  |
| [ ] | `object.datalayout_transfer` | M8 | P2 |  |
| [ ] | `object.join_uvs` | M8 | P2 |  |
| [ ] | `object.make_links_scene` | M8 | P2 |  |
| [ ] | `object.instance_offset_from_cursor` | M8 | P2 |  |
| [ ] | `object.instance_offset_from_object` | M8 | P2 |  |
| [ ] | `object.instance_offset_to_cursor` | M8 | P2 |  |
| [ ] | `object.copy_global_transform` | M8 | P2 |  |
| [ ] | `object.copy_relative_transform` | M8 | P2 |  |
| [ ] | `object.paste_transform` | M8 | P2 |  |
| [ ] | `object.unlink_data` | M8 | P2 |  |
| [ ] | `object.update_shapes` | M8 | P2 |  |
| [ ] | `object.visual_geometry_to_objects` | M8 | P2 |  |
| [ ] | `object.voxel_remesh` | M8 | P2 |  |
| [ ] | `object.modifier_convert` | M8 | P2 |  |
| — | `object.constraint_add` | OUT | — | constraints out of scope |
| — | `object.constraint_add_with_targets` | OUT | — |  |
| — | `object.constraints_clear` | OUT | — |  |
| — | `object.constraints_copy` | OUT | — |  |
| — | `object.vertex_parent_set` | OUT | — |  |
| — | `object.anim_transforms_to_deltas` | OUT | — |  |
| — | `object.shape_key_retime` | OUT | — |  |
| — | `object.empty_image_add` | OUT | — |  |
| — | `object.data_instance_add` | OUT | — |  |
| — | `object.text_add` | OUT | — |  |
| — | `object.make_local` | OUT | — | library linking |

### `transform.*`

| ✓ | Operator | M | Prio | Notes |
|---|---|---|---|---|
| [x] | `transform.translate` | M1 | P0 | object mode M1; edit M2; pose M6 (mode handlers, dispatcher owned by WP 1.21) |
| [x] | `transform.rotate` | M1 | P0 | +θ about axis (verified) |
| [x] | `transform.resize` | M1 | P0 | decomposes matrices, loses shear (as Blender) |
| [x] | `transform.mirror` | M2 | P1 | Edit Mode (meshes) only |
| [x] | `transform.transform` | M2 | P1 | generic `mode` dispatcher; WP 2.12; Object Mode CREASE/BWEIGHT raise NotImplementedError (Blender 5.2.4 crashes) |
| [x] | `transform.shrink_fatten` | M2 | P1 |  |
| [x] | `transform.edge_slide` | M2 | P1 |  |
| [x] | `transform.edge_crease` | M2 | P1 | writes crease_edge |
| [x] | `transform.edge_bevelweight` | M2 | P1 | writes bevel_weight_edge |
| [x] | `transform.tosphere` | M2 | P1 | Edit Mode (meshes) only |
| [x] | `transform.vert_slide` | M2 | P2 |  |
| [x] | `transform.vert_crease` | M2 | P2 |  |
| [x] | `transform.push_pull` | M2 | P2 | Edit Mode (meshes) only |
| [x] | `transform.vertex_random` | M2 | P2 | BliRng |
| [ ] | `transform.shear` | M8 | P2 |  |
| [ ] | `transform.bend` | M8 | P2 |  |
| [ ] | `transform.rotate_normal` | M8 | P2 |  |
| [ ] | `transform.create_orientation` | M8 | P2 |  |
| [ ] | `transform.select_orientation` | M8 | P2 |  |
| [ ] | `transform.delete_orientation` | M8 | P2 |  |
| — | `transform.trackball` | OUT | — |  |
| — | `transform.vertex_warp` | OUT | — |  |
| — | `transform.skin_resize` | OUT | — |  |
| — | `transform.tilt` | OUT | — | curves |
| — | `transform.bbone_resize` | OUT | — |  |
| — | `transform.from_gizmo` | OUT | — | interactive |
| — | `transform.seq_slide` | OUT | — | sequencer |

### `view3d.* (cursor, snapping, camera)`

| ✓ | Operator | M | Prio | Notes |
|---|---|---|---|---|
| [x] | `view3d.snap_cursor_to_active` | M1 | P0 | virtual 3D View poll |
| [x] | `view3d.snap_cursor_to_center` | M1 | P0 |  |
| [x] | `view3d.snap_cursor_to_selected` | M1 | P0 |  |
| [x] | `view3d.snap_selected_to_cursor` | M1 | P0 |  |
| [x] | `view3d.snap_cursor_to_grid` | M1 | P1 |  |
| [x] | `view3d.snap_selected_to_active` | M1 | P1 |  |
| [x] | `view3d.snap_selected_to_grid` | M1 | P2 |  |
| [ ] | `view3d.camera_to_view_selected` | M3 | P0 | frames selected objects with scene.camera |
| [ ] | `view3d.object_as_camera` | M3 | P1 | WP 3.4 (M3+M4 W0 assignment) |

### `uv.*`

| ✓ | Operator | M | Prio | Notes |
|---|---|---|---|---|
| [ ] | `uv.unwrap` | M3 | P0 | default CONFORMAL (LSCM); ANGLE_BASED/MINIMUM_STRETCH exact in M8 |
| [ ] | `uv.smart_project` | M3 | P0 |  |
| [ ] | `uv.pack_islands` | M3 | P0 | T3 invariants |
| [ ] | `uv.cube_project` | M3 | P1 |  |
| [ ] | `uv.cylinder_project` | M3 | P1 | headless view fallback as Blender |
| [ ] | `uv.sphere_project` | M3 | P1 |  |
| [ ] | `uv.reset` | M3 | P1 |  |
| [ ] | `uv.average_islands_scale` | M3 | P1 |  |
| [ ] | `uv.seams_from_islands` | M3 | P1 |  |
| [ ] | `uv.mark_seam` | M3 | P1 |  |
| [ ] | `uv.follow_active_quads` | M3 | P2 | needs active face |
| [ ] | `uv.lightmap_pack` | M3 | P2 | upper-case prop names are real |
| [ ] | `uv.pin` | M3 | P2 |  |
| [ ] | `uv.project_from_view` | M3 | P2 | frap-extended: projects from scene.camera |
| [ ] | `uv.minimize_stretch` | M8 | P2 | iterations > 0 headless |
| [ ] | `uv.select_all` | M8 | P2 | meaningful with sync select off |
| [ ] | `uv.select_mode` | M8 | P2 |  |
| [ ] | `uv.select_linked` | M8 | P2 |  |
| [ ] | `uv.select_more` | M8 | P2 |  |
| [ ] | `uv.select_less` | M8 | P2 |  |
| [ ] | `uv.select_split` | M8 | P2 |  |
| [ ] | `uv.select_overlap` | M8 | P2 |  |
| [ ] | `uv.select_similar` | M8 | P2 |  |
| [ ] | `uv.select_pinned` | M8 | P2 |  |
| [ ] | `uv.select_by_winding` | M8 | P2 |  |
| [ ] | `uv.shortest_path_select` | M8 | P2 |  |
| [ ] | `uv.hide` | M8 | P2 |  |
| [ ] | `uv.reveal` | M8 | P2 |  |
| [ ] | `uv.weld` | M8 | P2 |  |
| [ ] | `uv.remove_doubles` | M8 | P2 |  |
| [ ] | `uv.stitch` | M8 | P2 |  |
| [ ] | `uv.align` | M8 | P2 |  |
| [ ] | `uv.align_rotation` | M8 | P2 |  |
| [ ] | `uv.arrange_islands` | M8 | P2 |  |
| [ ] | `uv.randomize_uv_transform` | M8 | P2 |  |
| [ ] | `uv.copy_mirrored_faces` | M8 | P2 |  |
| [ ] | `uv.snap_selected` | M8 | P2 |  |
| [ ] | `uv.snap_cursor` | M8 | P2 |  |
| [ ] | `uv.move_on_axis` | M8 | P2 |  |
| [ ] | `uv.rip` | M8 | P2 |  |
| [ ] | `uv.rip_move` | M8 | P2 |  |
| [ ] | `uv.export_layout` | M8 | P2 |  |
| — | `uv.select` | OUT | — | mouse pick |
| — | `uv.select_box` | OUT | — |  |
| — | `uv.select_circle` | OUT | — |  |
| — | `uv.select_lasso` | OUT | — |  |
| — | `uv.select_loop` | OUT | — |  |
| — | `uv.select_edge_ring` | OUT | — |  |
| — | `uv.select_linked_pick` | OUT | — |  |
| — | `uv.select_tile` | OUT | — |  |
| — | `uv.shortest_path_pick` | OUT | — |  |
| — | `uv.cursor_set` | OUT | — |  |
| — | `uv.custom_region_set` | OUT | — |  |
| — | `uv.copy` | OUT | — | clipboard |
| — | `uv.paste` | OUT | — | clipboard |

### `anim.*`

| ✓ | Operator | M | Prio | Notes |
|---|---|---|---|---|
| [ ] | `anim.keyframe_insert` | M6 | P0 | keying-set operator; scripts usually use bpy_struct.keyframe_insert |
| [ ] | `anim.keyframe_insert_by_name` | M6 | P1 |  |
| [ ] | `anim.keyframe_delete` | M6 | P1 |  |
| [ ] | `anim.keyframe_delete_v3d` | M6 | P1 |  |
| [ ] | `anim.keyframe_clear_v3d` | M6 | P1 |  |
| [ ] | `anim.keyframe_delete_by_name` | M6 | P2 |  |
| [ ] | `anim.keying_set_active_set` | M8 | P2 |  |
| [ ] | `anim.channels_bake` | M8 | P2 |  |

### `screen.* (frames)`

| ✓ | Operator | M | Prio | Notes |
|---|---|---|---|---|
| [ ] | `screen.frame_jump` | M6 | P1 |  |
| [ ] | `screen.frame_offset` | M6 | P1 |  |
| [ ] | `screen.keyframe_jump` | M6 | P2 |  |

### `armature.* (edit bones)`

| ✓ | Operator | M | Prio | Notes |
|---|---|---|---|---|
| [ ] | `armature.bone_primitive_add` | M6 | P1 |  |
| [ ] | `armature.extrude` | M6 | P1 |  |
| [ ] | `armature.extrude_move` | M6 | P1 | macro |
| [ ] | `armature.duplicate` | M6 | P1 |  |
| [ ] | `armature.duplicate_move` | M6 | P1 | macro |
| [ ] | `armature.delete` | M6 | P1 |  |
| [ ] | `armature.subdivide` | M6 | P1 |  |
| [ ] | `armature.parent_set` | M6 | P1 |  |
| [ ] | `armature.parent_clear` | M6 | P1 |  |
| [ ] | `armature.select_all` | M6 | P1 |  |
| [ ] | `armature.calculate_roll` | M6 | P1 |  |
| [ ] | `armature.roll_clear` | M6 | P1 |  |
| [ ] | `armature.switch_direction` | M6 | P1 |  |
| [ ] | `armature.symmetrize` | M6 | P1 |  |
| [ ] | `armature.extrude_forked` | M8 | P2 |  |
| [ ] | `armature.dissolve` | M8 | P2 |  |
| [ ] | `armature.split` | M8 | P2 |  |
| [ ] | `armature.separate` | M8 | P2 |  |
| [ ] | `armature.fill` | M8 | P2 |  |
| [ ] | `armature.align` | M8 | P2 |  |
| [ ] | `armature.select_more` | M8 | P2 |  |
| [ ] | `armature.select_less` | M8 | P2 |  |
| [ ] | `armature.select_hierarchy` | M8 | P2 |  |
| [ ] | `armature.select_linked` | M8 | P2 |  |
| [ ] | `armature.select_mirror` | M8 | P2 |  |
| [ ] | `armature.select_similar` | M8 | P2 |  |
| [ ] | `armature.hide` | M8 | P2 |  |
| [ ] | `armature.reveal` | M8 | P2 |  |
| [ ] | `armature.flip_names` | M8 | P2 |  |
| [ ] | `armature.autoside_names` | M8 | P2 |  |
| [ ] | `armature.duplicate_rename` | M8 | P2 |  |
| [ ] | `armature.collection_add` | M8 | P2 | bone collections |
| [ ] | `armature.collection_remove` | M8 | P2 |  |
| [ ] | `armature.collection_assign` | M8 | P2 |  |
| [ ] | `armature.collection_unassign` | M8 | P2 |  |
| [ ] | `armature.assign_to_collection` | M8 | P2 |  |
| [ ] | `armature.move_to_collection` | M8 | P2 |  |
| — | `armature.click_extrude` | OUT | — | interactive |
| — | `armature.select_linked_pick` | OUT | — |  |
| — | `armature.shortest_path_pick` | OUT | — |  |

### `pose.*`

| ✓ | Operator | M | Prio | Notes |
|---|---|---|---|---|
| [ ] | `pose.select_all` | M6 | P1 |  |
| [ ] | `pose.transforms_clear` | M6 | P1 |  |
| [ ] | `pose.loc_clear` | M6 | P1 |  |
| [ ] | `pose.rot_clear` | M6 | P1 |  |
| [ ] | `pose.scale_clear` | M6 | P1 |  |
| [ ] | `pose.armature_apply` | M6 | P1 | apply pose as rest pose |
| [ ] | `pose.rotation_mode_set` | M6 | P1 |  |
| [ ] | `pose.user_transforms_clear` | M8 | P2 |  |
| [ ] | `pose.visual_transform_apply` | M8 | P2 |  |
| [ ] | `pose.copy` | M8 | P2 |  |
| [ ] | `pose.paste` | M8 | P2 |  |
| [ ] | `pose.select_hierarchy` | M8 | P2 |  |
| [ ] | `pose.select_parent` | M8 | P2 |  |
| [ ] | `pose.select_linked` | M8 | P2 |  |
| [ ] | `pose.select_mirror` | M8 | P2 |  |
| [ ] | `pose.select_grouped` | M8 | P2 |  |
| [ ] | `pose.hide` | M8 | P2 |  |
| [ ] | `pose.reveal` | M8 | P2 |  |
| [ ] | `pose.flip_names` | M8 | P2 |  |
| [ ] | `pose.autoside_names` | M8 | P2 |  |
| [ ] | `pose.quaternions_flip` | M8 | P2 |  |
| — | `pose.constraint_add` | OUT | — | constraints out of scope |
| — | `pose.ik_add` | OUT | — |  |
| — | `pose.breakdown` | OUT | — | interactive pose tools |
| — | `pose.push` | OUT | — |  |
| — | `pose.relax` | OUT | — |  |

### `nla.*`

| ✓ | Operator | M | Prio | Notes |
|---|---|---|---|---|
| [ ] | `nla.action_pushdown` | M6 | P1 | headless-extended: active object's AnimData; feeds NLA_TRACKS export |
| [ ] | `nla.tracks_add` | M6 | P2 | headless-extended |
| [ ] | `nla.tracks_delete` | M8 | P2 |  |
| [ ] | `nla.actionclip_add` | M8 | P2 |  |

### `render.*`

| ✓ | Operator | M | Prio | Notes |
|---|---|---|---|---|
| [x] | `render.render` | M1 | P0 | still M1 (Workbench); EEVEE M5; animation=True M6 |
| [ ] | `render.opengl` | M8 | P2 | maps to Workbench from scene camera |

### `wm.* (files)`

| ✓ | Operator | M | Prio | Notes |
|---|---|---|---|---|
| [x] | `wm.open_mainfile` | M1 | P0 | .frap only |
| [x] | `wm.save_as_mainfile` | M1 | P0 | .frap only |
| [x] | `wm.save_mainfile` | M1 | P0 |  |
| [x] | `wm.read_homefile` | M1 | P0 | use_empty |
| [x] | `wm.read_factory_settings` | M1 | P1 |  |
| [x] | `wm.revert_mainfile` | M1 | P2 |  |
| — | `wm.obj_import` | OUT | — | import is glTF/GLB only |
| — | `wm.fbx_import` | OUT | — |  |
| — | `wm.usd_import` | OUT | — |  |
| — | `wm.stl_import` | OUT | — |  |
| — | `wm.ply_import` | OUT | — |  |
| — | `wm.alembic_import` | OUT | — |  |

### `scene.*`

| ✓ | Operator | M | Prio | Notes |
|---|---|---|---|---|
| [ ] | `scene.new` | M3 | P1 | WP 3.10; multi-scene glTF |
| [ ] | `scene.delete` | M3 | P2 | WP 3.10 |
| — | `scene.view_layer_add` | OUT | — |  |
| — | `scene.view_layer_remove` | OUT | — |  |

### `material.*`

| ✓ | Operator | M | Prio | Notes |
|---|---|---|---|---|
| [x] | `material.new` | M1 | P0 | does not assign to a slot from scripts (as Blender) |
| [ ] | `material.copy` | M8 | P2 |  |
| [ ] | `material.paste` | M8 | P2 |  |

### `image.*`

| ✓ | Operator | M | Prio | Notes |
|---|---|---|---|---|
| [ ] | `image.new` | M3 | P0 |  |
| [ ] | `image.open` | M3 | P0 |  |
| [ ] | `image.pack` | M3 | P1 |  |
| [ ] | `image.unpack` | M3 | P2 |  |
| [ ] | `image.reload` | M3 | P2 |  |
| [ ] | `image.save` | M3 | P2 |  |
| [ ] | `image.save_as` | M3 | P2 |  |

### `world.*`

| ✓ | Operator | M | Prio | Notes |
|---|---|---|---|---|
| [ ] | `world.new` | M3 | P1 |  |

### `collection.*`

| ✓ | Operator | M | Prio | Notes |
|---|---|---|---|---|
| [ ] | `collection.create` | M8 | P2 |  |
| [ ] | `collection.objects_add_active` | M8 | P2 |  |
| [ ] | `collection.objects_remove` | M8 | P2 |  |
| [ ] | `collection.objects_remove_all` | M8 | P2 |  |
| [ ] | `collection.objects_remove_active` | M8 | P2 |  |

### `import_scene.*`

| ✓ | Operator | M | Prio | Notes |
|---|---|---|---|---|
| [x] | `import_scene.gltf` | M1 | P0 | static M1; materials/cameras/lights, scenes, strips/fans/lines/points, `import_point_as_pointcloud` M3; anim/skin/morph M6; Draco/meshopt/KTX2 M7 |
| — | `import_scene.fbx` | OUT | — | import is glTF/GLB only |

### `export_scene.*`

| ✓ | Operator | M | Prio | Notes |
|---|---|---|---|---|
| [x] | `export_scene.gltf` | M1 | P0 | Blender's 110 props + frap_* options (only where Blender has no prop); feature milestones as import; `//` resolved (deviation) |
| — | `export_scene.fbx` | OUT | — |  |

### `ed.* (undo)`

| ✓ | Operator | M | Prio | Notes |
|---|---|---|---|---|
| [ ] | `ed.undo` | M8 | P2 | WP 8.4: the snapshot ring of committed `State`s |
| [ ] | `ed.redo` | M8 | P2 | WP 8.4 |
| [ ] | `ed.undo_redo` | M8 | P2 | WP 8.4 |
| [ ] | `ed.undo_push` | M8 | P2 | WP 8.4 |

### `frap.*` (frap extensions, namespaced)

| ✓ | Operator | M | Notes |
|---|---|---|---|
| [x] | `frap.describe` | M2 | target (op:<idname>, struct:<Struct>, <Struct>.<prop>, errors, protocol, manifest) → manifest fragment in `result.describe`; T0; served through `frap_op::DESCRIBE_PROVIDERS` (`frap-schema/src/provider.rs` registers the manifest); unknown target → `KeyError` `DESCRIBE_TARGET`, missing/`None`/empty target → `TypeError` `OP_ARG_TYPE`; WP 2.13 |
| [x] | `frap.select_elements` | M2 | prereq (WP 2.2); type VERT/EDGE/FACE, indices, action SET/ADD/REMOVE, object; agent selection by index |
| [x] | `frap.select_by_normal` | M2 | direction, threshold, extend; select faces facing a direction |
| [x] | `frap.scene_summary` | M2 | objects, counts, bounds, materials, modifiers in one JSON value |
| [x] | `frap.render_contact_sheet` | M2 | multi-view sheet (FRONT/BACK/LEFT/RIGHT/TOP/BOTTOM/ISO/CAMERA); synthetic cameras never enter bpy.data |

## B. Modifier types (`object.modifier_add(type=…)`, `modifiers.new(name, type)`)

All types exist as data from M1 (settings generated from dumps, round-trip in `.frap`); the milestone is when the **evaluator** lands. Types without an evaluator pass the mesh through with a WARNING.

| ✓ | Type | Struct | M | Prio | Notes |
|---|---|---|---|---|---|
| [ ] | `SUBSURF` | `SubsurfModifier` | M4 | P0 | frap-subdiv; T2, layout pass towards T1 |
| [ ] | `MIRROR` | `MirrorModifier` | M4 | P0 |  |
| [ ] | `ARRAY` | `ArrayModifier` | M4 | P0 |  |
| [ ] | `BEVEL` | `BevelModifier` | M4 | P0 | shares mesh.bevel core |
| [ ] | `SOLIDIFY` | `SolidifyModifier` | M4 | P0 | simple + complex |
| [ ] | `BOOLEAN` | `BooleanModifier` | M4 | P0 | MANIFOLD T2, EXACT T3, FLOAT → EXACT + WARNING until M8 |
| [ ] | `TRIANGULATE` | `TriangulateModifier` | M4 | P0 |  |
| [ ] | `DECIMATE` | `DecimateModifier` | M4 | P0 | COLLAPSE/UNSUBDIV/DISSOLVE |
| [ ] | `WELD` | `WeldModifier` | M4 | P0 |  |
| [ ] | `DISPLACE` | `DisplaceModifier` | M4 | P0 | Texture IMAGE/NONE |
| [ ] | `NODES` | `NodesModifier` | M4 | P0 | "Smooth by Angle" only; inputs at properties.inputs.Input_1.value |
| [ ] | `EDGE_SPLIT` | `EdgeSplitModifier` | M4 | P1 |  |
| [ ] | `WEIGHTED_NORMAL` | `WeightedNormalModifier` | M4 | P1 |  |
| [ ] | `SMOOTH` | `SmoothModifier` | M4 | P1 |  |
| [ ] | `CAST` | `CastModifier` | M4 | P1 |  |
| [ ] | `SIMPLE_DEFORM` | `SimpleDeformModifier` | M4 | P1 |  |
| [ ] | `WIREFRAME` | `WireframeModifier` | M4 | P1 |  |
| [ ] | `SCREW` | `ScrewModifier` | M4 | P1 |  |
| [ ] | `MASK` | `MaskModifier` | M4 | P1 |  |
| [ ] | `BUILD` | `BuildModifier` | M4 | P2 |  |
| [ ] | `NORMAL_EDIT` | `NormalEditModifier` | M4 | P2 |  |
| [ ] | `LAPLACIANSMOOTH` | `LaplacianSmoothModifier` | M4 | P2 |  |
| [ ] | `ARMATURE` | `ArmatureModifier` | M6 | P0 | LBS; dual quaternion P2 |
| [ ] | `REMESH` | `RemeshModifier` | M8 | P2 |  |
| [ ] | `SHRINKWRAP` | `ShrinkwrapModifier` | M8 | P2 |  |
| [ ] | `UV_PROJECT` | `UVProjectModifier` | M8 | P2 |  |
| [ ] | `UV_WARP` | `UVWarpModifier` | M8 | P2 |  |
| [ ] | `CORRECTIVE_SMOOTH` | `CorrectiveSmoothModifier` | M8 | P2 |  |
| [ ] | `WAVE` | `WaveModifier` | M8 | P2 |  |
| [ ] | `WARP` | `WarpModifier` | M8 | P2 |  |
| [ ] | `DATA_TRANSFER` | `DataTransferModifier` | M8 | P2 |  |
| [ ] | `VERTEX_WEIGHT_EDIT` | `VertexWeightEditModifier` | M8 | P2 |  |
| [ ] | `VERTEX_WEIGHT_MIX` | `VertexWeightMixModifier` | M8 | P2 |  |
| [ ] | `VERTEX_WEIGHT_PROXIMITY` | `VertexWeightProximityModifier` | M8 | P2 |  |
| — | `SKIN` | `SkinModifier` | OUT | — | data only |
| — | `CURVE` | `CurveModifier` | OUT | — | no curve objects |
| — | `LATTICE` | `LatticeModifier` | OUT | — | no lattice objects |
| — | `MULTIRES` | `MultiresModifier` | OUT | — |  |
| — | `HOOK` | `HookModifier` | OUT | — |  |
| — | `LAPLACIANDEFORM` | `LaplacianDeformModifier` | OUT | — |  |
| — | `MESH_DEFORM` | `MeshDeformModifier` | OUT | — |  |
| — | `SURFACE_DEFORM` | `SurfaceDeformModifier` | OUT | — |  |
| — | `MESH_CACHE` | `MeshCacheModifier` | OUT | — |  |
| — | `MESH_SEQUENCE_CACHE` | `MeshSequenceCacheModifier` | OUT | — |  |
| — | `VOLUME_TO_MESH` | `VolumeToMeshModifier` | OUT | — |  |

## C. Shader node types (`node_tree.nodes.new(type=…)`)

Every dumped `ShaderNode*` type can be created and edited as data from M1 (generic `NodeTypeDef`). The milestone below is when `frap-material` lowering (glTF export) and the renderer understand it. Removed 4.x types (`ShaderNodeSeparateRGB`, …) raise Blender's `Node type … undefined`.

| ✓ | bl_idname | Default name | M | glTF role |
|---|---|---|---|---|
| [ ] | `ShaderNodeBsdfPrincipled` | Principled BSDF | M1 | pbrMetallicRoughness + all KHR_materials_* (M3) |
| [ ] | `ShaderNodeOutputMaterial` | Material Output | M1 | root |
| [ ] | `ShaderNodeTexImage` | Image Texture | M1 | textures, samplers (interpolation, extension) |
| [ ] | `ShaderNodeMix` | Mix | M3 | vertex-colour and factor multiplies |
| [ ] | `ShaderNodeNormalMap` | Normal Map | M3 | normalTexture + scale |
| [ ] | `ShaderNodeBump` | Bump | M3 | read only (not exported, as Blender) |
| [ ] | `ShaderNodeMapping` | Mapping | M3 | KHR_texture_transform |
| [ ] | `ShaderNodeTexCoord` | Texture Coordinate | M3 | UV source |
| [ ] | `ShaderNodeUVMap` | UV Map | M3 | texCoord index |
| [ ] | `ShaderNodeVertexColor` | Color Attribute | M3 | COLOR_n |
| [ ] | `ShaderNodeAttribute` | Attribute | M3 | COLOR_n / custom |
| [ ] | `ShaderNodeSeparateColor` | Separate Color | M3 | channel selection (ORM) |
| [ ] | `ShaderNodeCombineColor` | Combine Color | M3 | channel packing on import |
| [ ] | `ShaderNodeSeparateXYZ` | Separate XYZ | M3 |  |
| [ ] | `ShaderNodeCombineXYZ` | Combine XYZ | M3 |  |
| [ ] | `ShaderNodeMath` | Math | M3 | alpha MASK patterns (ROUND, compare) |
| [ ] | `ShaderNodeVectorMath` | Vector Math | M3 |  |
| [ ] | `ShaderNodeRGB` | RGB | M3 | constant factors |
| [ ] | `ShaderNodeValue` | Value | M3 | constant factors |
| [ ] | `ShaderNodeInvert` | Invert | M3 |  |
| [ ] | `ShaderNodeEmission` | Emission | M3 | emissive, unlit patterns |
| [ ] | `ShaderNodeBsdfTransparent` | Transparent BSDF | M3 | alpha / unlit patterns |
| [ ] | `ShaderNodeMixShader` | Mix Shader | M3 | alpha / unlit patterns |
| [ ] | `ShaderNodeAddShader` | Add Shader | M3 |  |
| [ ] | `ShaderNodeLightPath` | Light Path | M3 | KHR_materials_unlit pattern |
| [ ] | `ShaderNodeVolumeAbsorption` | Volume Absorption | M3 | KHR_materials_volume |
| [ ] | `ShaderNodeGroup` | Group | M3 | "glTF Material Output" (Occlusion, Thickness, …) |
| [ ] | `ShaderNodeBackground` | Background | M3 | world colour (render M5) |
| [ ] | `ShaderNodeTexEnvironment` | Environment Texture | M3 | world HDRI (render M5) |
| [ ] | `ShaderNodeOutputWorld` | World Output | M3 | world root |

## D. RNA structs

A struct is done when its `StructDef` is generated, its bindings pass `check_bindings`, and golden cases read/write its listed properties through `tests/golden/props.toml`. Properties not bound yet raise `NotImplementedError` naming the struct and property.

| ✓ | Struct(s) | M | Notes |
|---|---|---|---|
| [ ] | ID (name, users, use_fake_user, rename(), session_uid, library, is_linked_packed, is_embedded_data, custom properties) | M1 | names ≤255 bytes; unique per (name, library); embedded IDs via owner |
| [ ] | Library | M1 | minimal: name, filepath, parent, is_archive; created by built-in linked groups (M4 use) |
| [ ] | Window (stub: scene, view_layer) | M1 read / M3 write | `bpy.context.window.scene = …` switches scenes |
| [ ] | AddonPreferences for `io_scene_gltf2` (`KHR_materials_variants_ui`, …) | M1 data / M7 variants | via `bpy.context.preferences.addons['io_scene_gltf2'].preferences` |
| [ ] | BlendData + BlendData<Coll> (objects, meshes, materials, cameras, lights, collections, worlds, scenes, images, node_groups, libraries, pointclouds) | M1 | new/remove/get/find/[i]/['name']; order rule §5.4; `scenes.new` M3 |
| [ ] | Context (object, active_object, selected_objects, selected_editable_objects, visible_objects, selectable_objects, editable_objects, scene, view_layer, collection, tool_settings, mode, edit_object, objects_in_mode*, blend_data) | M1 | read-only |
| [x] | Context (preferences, space_data stub) | M2 | WP 2.3 (`frap-session/src/resolve/context/ui.rs`): `window`, `space_data` (`SpaceView3D` stub: `type`, `shading`, read-only `region_3d`), factory `preferences`; `screen`/`area`/`region` read `None` (deviations D-61, D-62) |
| [ ] | Object | M1 | transforms, parent, matrix_world (fresh), dimensions, data, type, mode, material_slots, hide_*, display; vertex_groups M6; pose M6; animation_data M6 |
| [ ] | ObjectModifiers, Modifier (+ every <Type>Modifier settings struct) | M1 data / M4 eval | generated settings |
| [ ] | Mesh, MeshVertex, MeshEdge, MeshPolygon, MeshLoop, MeshLoopTriangle | M1 | SoA proxies (WP 1.27); use_smooth/use_seam/use_edge_sharp proxies; foreach_get/foreach_set; edit-mode reads via cached view (M2) |
| [ ] | AttributeGroupMesh, Attribute and typed attributes (FloatAttribute, FloatVectorAttribute, FloatColorAttribute, ByteColorAttribute, BoolAttribute, IntAttribute, Float2Attribute, …) | M1 | attributes.new/remove, active_color, default_color |
| [ ] | MeshUVLoopLayer, MeshUVLoop, UVLoopLayers | M1 | uv_layers.new(do_init), active, active_render |
| [ ] | Material, MaterialSlot, IDMaterials (mesh.materials) | M1 | diffuse_color (Workbench), surface_render_method, use_backface_culling; MaterialSlot derived from data + per-object overrides |
| [ ] | PointCloud | M1 data / M3 glTF | POINTS export, `import_point_as_pointcloud` |
| [ ] | ShaderNodeTree, Node, NodeSocket (+ typed sockets), NodeLink, Nodes, NodeLinks | M1 | first-enabled socket lookup; generic for all node types; material/world trees are embedded IDs |
| [ ] | Camera, CameraDOFSettings | M1 | lens/angle hook; glTF M3 |
| [ ] | Light, PointLight, SunLight, SpotLight, AreaLight | M1 | type re-typing hook; glTF M3 |
| [ ] | World | M1 | node tree; HDRI render M5 |
| [ ] | Scene, RenderSettings, ImageFormatSettings, ColorManagedViewSettings, ColorManagedDisplaySettings | M1 | generated plain structs |
| [ ] | SceneDisplay, View3DShading | M1 | Workbench settings |
| [ ] | SceneEEVEE | M1 data / M5 render |  |
| [ ] | ToolSettings (mesh_select_mode, transform_pivot_point, proportional_*, double_threshold, use_uv_select_sync, vertex_group_weight) | M1 data / M2 use | subset |
| [ ] | View3DCursor, UnitSettings | M1 |  |
| [ ] | ViewLayer, LayerObjects, LayerCollection | M1 | bases, active, selection per view layer |
| [ ] | Collection, CollectionObjects, CollectionChildren | M1 | link/unlink |
| [ ] | Depsgraph, DepsgraphObjectInstance (evaluated_get, to_mesh) | M1 | always fresh |
| [ ] | Image, ImagePackedFile | M1 meta / M3 full | pixels, pack, save, save_render |
| [ ] | Texture, ImageTexture | M4 | DISPLACE only; field set frozen by M3+M4 W0 (`TextureFields`), bindings and `bpy.data.textures.new` WP 4.2 |
| [ ] | VertexGroup, VertexGroups, VertexGroupElement | M6 | names on Mesh, RNA on Object |
| [ ] | Key, ShapeKey | M6 |  |
| [ ] | Armature, Bone, EditBone, ArmatureEditBones, Pose, PoseBone, BoneCollection | M6 |  |
| [ ] | Action, ActionSlot, ActionLayer, ActionStrip, ActionKeyframeStrip, ActionChannelbag, FCurve, Keyframe, FCurveKeyframePoints, ActionGroup | M6 | slotted actions; action.fcurves → 5.x hint |
| [ ] | AnimData, NlaTrack, NlaTracks, NlaStrip, NlaStrips | M6 |  |
| [x] | Preferences (edit.use_duplicate_*, edit.keyframe_new_*) | M2 read-only | moved from M1 (M1 stabilization) with `bpy.context.preferences` |

## E. RNA functions and Python-only members

| ✓ | Call | M | Notes |
|---|---|---|---|
| [ ] | `bpy.data.<coll>.new(...) / .remove(id, do_unlink=True) / .get / .find` | M1 | objects.new(name, data) is unlinked until collection.objects.link |
| [ ] | `Collection.objects.link/unlink, Collection.children.link/unlink, scene.collection` | M1 |  |
| [ ] | `Object.select_set/select_get/hide_set/hide_get/visible_get` | M1 | per view layer |
| [ ] | `view_layer.objects.active = …, view_layer.update()` | M1 |  |
| [ ] | `Object.evaluated_get(depsgraph), Object.to_mesh()/to_mesh_clear(), context.evaluated_depsgraph_get()` | M1 |  |
| [ ] | `Object.children / children_recursive / users_collection / users_scene (Python-only)` | M1 |  |
| [ ] | `ID.rename(name, mode), ID.copy(), ID.user_clear()` | M1 |  |
| [ ] | `Mesh.from_pydata(vertices, edges, faces), Mesh.shade_smooth()/shade_flat() (Python-only)` | M1 |  |
| [ ] | `Mesh.update(), validate(), calc_loop_triangles(), transform(matrix), clear_geometry()` | M1 | `validate()` answers `False` for a valid mesh and refuses to repair (`RNA_NOT_IMPLEMENTED`); `transform` ignores `shape_keys` (M6) |
| [x] | `Mesh.set_sharp_from_angle(angle), flip_normals(), split_faces()` | M2 | moved from M1 (M1 stabilization); `object.shade_smooth_by_angle` covers sharp-from-angle meanwhile; WP 2.9 (`frap-ops-data/src/funcs/mesh_*.rs`, M2 W0 assignment) |
| [ ] | `Mesh.calc_tangents(uvmap)` | M3 | bevy_mikktspace (`frap-geom/src/mikktspace.rs`); WP 3.7 (M3+M4 W0 assignment) |
| [ ] | `mesh.materials.append/pop/clear, MaterialSlot.material/link` | M1 |  |
| [ ] | `mesh.attributes.new/remove, mesh.uv_layers.new, mesh.color_attributes.new/remove` | M1 | colour attributes default to white and become active/default when none is |
| [x] | `mesh.uv_layers.remove, mesh.vertex_colors.new` | M2 | moved from M1 (M1 stabilization); `attributes.remove` removes a UV map meanwhile; vertex_colors creates "Col" BYTE_COLOR CORNER; WP 2.9 (`funcs/{u_v_loop_layers_remove,loop_colors_new}.rs`, M2 W0 assignment) |
| [ ] | `node_tree.nodes.new/remove, node_tree.links.new/remove` | M1 | links.new replaces existing input link |
| [ ] | `obj.modifiers.new(name, type)/remove/move/clear` | M4 (data from M1) | WP 4.1 (`funcs/object_modifiers_*.rs`); M4 golden prerequisite |
| [ ] | `Image.pack/unpack/save/save_render/scale/reload, bpy.data.images.load` | M3 |  |
| [ ] | `Scene.frame_set(frame, subframe=0.0)` | M6 | writes animated values to originals |
| [ ] | `bpy_struct.keyframe_insert(data_path, *, index=-1, frame=None, group="", options=set(), keytype='KEYFRAME')` | M6 | keyword-only after data_path |
| [ ] | `bpy_struct.keyframe_delete(data_path, *, index=-1, frame=None, group="")` | M6 |  |
| [ ] | `action.layers/strips/channelbag(slot, ensure=False), channelbag.fcurves.new/remove, fcurve.keyframe_points.insert/add` | M6 |  |
| [ ] | `AnimData: animation_data_create(), action, action_slot; nla_tracks.new(), track.strips.new(name, start, action)` | M6 |  |
| [ ] | `obj.vertex_groups.new/remove, VertexGroup.add(index, weight, type)/remove/weight(index)` | M6 | works in object mode |
| [ ] | `Object.shape_key_add(name, from_mix)/shape_key_remove/shape_key_clear` | M6 |  |
| [ ] | `bpy_struct.path_resolve / path_from_id, repr` | M1 | byte-identical to Blender, incl. embedded roots |
| [ ] | `<collection>.foreach_get(attr[, seq]) / foreach_set(attr, seq)`, slices `coll[a:b]` | M1 | one-argument `foreach_get` is a frap extension; bulk `$array` encoding |
| [ ] | `bpy.ops.<ns>.<op>.poll() / get_rna_type() / idname() / idname_py()` | M1 | poll returns bool + `unmet` requirements |
| [ ] | `bpy.data.scenes.new(name) / remove`, `context.window.scene = …` | M3 | WP 3.10 |
| — | `bpy_struct.driver_add/driver_remove` | OUT | drivers out of scope |

## F. glTF features

| ✓ | Feature | Export M | Import M | Notes |
|---|---|---|---|---|
| [ ] | Static meshes, node hierarchy (TRS), multi-material primitives, NORMAL, TEXCOORD_0 | M1 | M1 | Blender-compat rules, looptris |
| [ ] | Multiple scenes (`scenes[]`, default `scene`, `use_active_scene`) | M3 | M3 | WP 3.10 |
| [ ] | Non-triangle modes: LINES (`use_mesh_edges`), POINTS (`use_mesh_vertices`, point clouds); import of strips, fans, line strips/loops | M3 | M3 | WP 3.7 |
| [ ] | pbrMetallicRoughness factors + baseColorTexture, doubleSided | M1 | M1 |  |
| [ ] | Multiple UV sets, vertex colours (COLOR_n, fake COLOR_0), TANGENT, extras | M3 | M3 |  |
| [ ] | All textures/samplers/images (PNG/JPEG passthrough, ORM packing), alpha modes OPAQUE/MASK/BLEND | M3 | M3 |  |
| [ ] | Cameras (perspective/orthographic) | M3 | M3 | export_cameras=False by default |
| [ ] | KHR_lights_punctual | M3 | M3 | export_lights=False by default; AREA warning |
| [ ] | KHR_texture_transform | M3 | M3 | marked required (Blender quirk) |
| [ ] | KHR_materials_emissive_strength, unlit | M3 | M3 |  |
| [ ] | KHR_materials_transmission, clearcoat, sheen, ior, specular, volume, iridescence, anisotropy, dispersion | M3 | M3 |  |
| [ ] | KHR_materials_pbrSpecularGlossiness | — | M3 | import only (archived) |
| [ ] | Animations (TRS, weights; ACTIONS/ACTIVE_ACTIONS/BROADCAST/NLA_TRACKS/SCENE; LINEAR/STEP/CUBICSPLINE, sampling) | M6 | M6 |  |
| [ ] | Skins (joints, inverse bind matrices, JOINTS_n/WEIGHTS_n) | M6 | M6 |  |
| [ ] | Morph targets (POSITION/NORMAL deltas, sparse, default weights) | M6 | M6 |  |
| [ ] | KHR_animation_pointer | M6 | M6 |  |
| [ ] | KHR_mesh_quantization | M7 | M7 | frap_mesh_quantization or gltfpack path |
| [ ] | EXT_meshopt_compression, KHR_meshopt_compression | M7 | M7 |  |
| [ ] | KHR_draco_mesh_compression | M7 | M7 | spike gate |
| [ ] | EXT_texture_webp | M7 (lossless M3) | M3 |  |
| [ ] | KHR_texture_basisu | M7 (feature basisu-encode) | M7 | frap_texture_format='KTX2' |
| [ ] | KHR_materials_variants | M7 | M7 | add-on data model; RNA exists only with `KHR_materials_variants_ui` on (dumped that way); import turns it on |
| [ ] | EXT_mesh_gpu_instancing | M7 (`export_gpu_instances`, Blender prop) | M7 | children of an Empty sharing a mesh (add-on rule) |
| [ ] | KHR_node_visibility, KHR_materials_diffuse_transmission | M7 (frap_*) | M7 | not produced by Blender |

## G. Totals

| Milestone | Operators |
|---|---|
| M1 | 64 |
| M2 | 124 |
| M3 | 31 |
| M4 | 15 |
| M6 | 54 |
| M8 | 187 |
| OUT | 61 |
| total listed | 536 |
