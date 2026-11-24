# frap operator and RNA checklist

Companion to `docs/ARCHITECTURE.md`. Every operator, RNA struct, function, modifier type and shader node in scope, with the milestone it lands in. Operator names were checked against the operator list of Blender 5.2.4 (`-b --factory-startup`) when this file was generated.

**Legend.** `M1`…`M8`: milestone (ARCHITECTURE §22). `M9`: post-M8 backlog (no milestone plan yet, decision log M8W0-3): `--describe` lists it as planned `M9` and `OP_NOT_IMPLEMENTED` says "planned after M8". `OUT`: not planned; Blender has it, frap answers `NotImplementedError` (`OP_NOT_IMPLEMENTED`) and `--describe` lists it as `not_implemented`. Priority `P0` MVP, `P1` needed for the fixed scope, `P2` parity/nice to have (from the research catalogues). *macro*: takes nested `<SUB>_OT_<op>={...}` props. *headless-extended*: frap does more than `blender -b` (documented in `docs/deviations.md`; goldens use `#@blender` emulations). Every implemented operator must also reach fidelity tier T0; target tiers above T0 are noted where decided. *prereq*: part of a milestone's golden prerequisite set (ARCHITECTURE §22.1), owned by a WP one wave earlier than the goldens that use it.

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
| [x] | `mesh.select_nth` | M8 | P2 | without an active element: the first selected one (Blender's `deselect_nth_active`); WP 8.3.1 |
| [x] | `mesh.select_boundary_loop_multi` | M8 | P2 | WP 8.3.1 |
| [x] | `mesh.select_similar` | M8 | P2 | threshold shared with `uv.select_similar` (`ToolSettings.select_thresh`); WP 8.3.1 |
| [x] | `mesh.select_similar_region` | M8 | P2 | WP 8.3.1 |
| [x] | `mesh.select_interior_faces` | M8 | P2 | WP 8.3.1 |
| [x] | `mesh.select_mirror` | M8 | P2 | spatial mirror table (`use_mirror_topology` not bound); WP 8.3.1 |
| [x] | `mesh.select_axis` | M8 | P2 | WP 8.3.1 |
| [x] | `mesh.select_ungrouped` | M8 | P2 | WP 8.3.1 |
| [x] | `mesh.select_by_pole_count` | M8 | P2 | WP 8.3.1 |
| [x] | `mesh.select_next_item` | M8 | P2 | WP 8.3.1 |
| [x] | `mesh.select_prev_item` | M8 | P2 | WP 8.3.1 |
| [x] | `mesh.select_linked_pick` | M8 | P2 | headless-extended: index; WP 8.3.1 |
| [x] | `mesh.shortest_path_pick` | M8 | P2 | headless-extended: index; WP 8.3.1 |
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
| [x] | `mesh.extrude_manifold` | M2 | P2 | macro; the translate step's `use_automerge_and_split` (`EDBM_automerge_and_split`, `BM_mesh_intersect_edges`) since M4 (WP 4.6), golden `macros/manifold.py` |
| [x] | `mesh.inset` | M2 | P0 | T1 |
| [x] | `mesh.bevel` | M2 | P0 | seg 1 T1, N segments T2; offset=0 no-op hint (NO_OP_DEFAULT). Partial: `profile_type='CUSTOM'` OP_NOT_IMPLEMENTED; harden_normals and Auto Merge applied (M5+M6 W0, M4 W0), D-80 |
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
| [x] | `mesh.decimate` | M4 | P2 | shares DECIMATE COLLAPSE (WP 4.5) |
| [x] | `mesh.intersect` | M4 | P2 | shares Boolean solver; `solver='FLOAT'` is `BM_mesh_intersect` (T2) since WP 8.6, goldens `edit/intersect_float/knife_*` |
| [x] | `mesh.intersect_boolean` | M4 | P2 | solver FLOAT/EXACT (5.x renamed FAST); FLOAT is `BM_mesh_intersect` (T2) since WP 8.6, goldens `edit/intersect_float/boolean_*` |
| [x] | `mesh.blend_from_shape` | M6 | P2 | shape keys; WP 6.3 (M5+M6 W0) |
| [x] | `mesh.shape_propagate_to_all` | M6 | P2 | shape keys; WP 6.3 (M5+M6 W0) |
| [x] | `mesh.normals_tools` | M8 | P2 | copy buffer `tool_settings.normal_vector`; COPY partial-failure rollback D-150; WP 8.3.2 |
| [x] | `mesh.smooth_normals` | M8 | P2 | WP 8.3.2 |
| [x] | `mesh.average_normals` | M8 | P2 | WP 8.3.2 |
| [x] | `mesh.point_normals` | M8 | P2 | headless-extended: points at `target_location` only (no mouse; `mode` ignored, as Blender's exec); WP 8.3.2 |
| [x] | `mesh.merge_normals` | M8 | P2 | WP 8.3.2 |
| [x] | `mesh.split_normals` | M8 | P2 | WP 8.3.2 |
| [x] | `mesh.mod_weighted_strength` | M8 | P2 | WP 8.3.2 |
| [x] | `mesh.colors_rotate` | M8 | P2 | WP 8.3.2 |
| [x] | `mesh.colors_reverse` | M8 | P2 | WP 8.3.2 |
| [x] | `mesh.uvs_rotate` | M8 | P2 | WP 8.3.2 |
| [x] | `mesh.uvs_reverse` | M8 | P2 | WP 8.3.2 |
| [x] | `mesh.sort_elements` | M8 | P2 | every type incl. RANDOMIZE per seed and `reverse`, libc `qsort` tie order; WP 8.3.3 (goldens `edit/tools_m8/sort_elements*`) |
| [x] | `mesh.reorder_vertices_spatial` | M8 | P2 | sculpt-BVH face partition (libc++ `std::partition`), material split; WP 8.3.3 (golden `edit/tools_m8/reorder_vertices_spatial`) |
| [x] | `mesh.rip` | M8 | P2 | headless-extended (D-153): explicit side and view, `frap_location` (the point the mouse is over; unset: the 3D cursor) and `frap_direction` (non-zero: an orthographic view looking along it, the mouse ray `frap_location` + t·`frap_direction`; zero: the factory Layout 3D View); WP 8.3.3 (goldens `edit/tools_m8/rip_edges`, `rip_verts`; 25 interactive recordings `edit/tools_m8/rip_recordings`; transcript `tests/cli/m8/tools_m8/rip_direction`) |
| [x] | `mesh.rip_move` | M8 | P2 | macro; headless-extended (D-153): `MESH_OT_rip={'frap_location': …, 'frap_direction': …}`; WP 8.3.3 |
| [x] | `mesh.rip_edge` | M8 | P2 | `location`/`direction` ray (exec); WP 8.3.3 (golden `edit/tools_m8/rip_edge`) |
| [x] | `mesh.rip_edge_move` | M8 | P2 | macro; WP 8.3.3 |
| [x] | `mesh.vertices_smooth_laplacian` | M8 | P2 | T2 (sparse least squares, faer LU; ill-conditioned, diverging runs only T3); WP 8.3.3 (goldens `edit/tools_m8/vertices_smooth_laplacian`, `vertices_smooth_laplacian_closed`) |
| [x] | `mesh.symmetry_snap` | M8 | P2 | KD-tree / topology mirror cache; WP 8.3.3 (golden `edit/tools_m8/symmetry_snap`) |
| [x] | `mesh.face_make_planar` | M8 | P2 | T2 (iterative); WP 8.3.3 (golden `edit/tools_m8/face_make_planar`) |
| [x] | `mesh.circularize` | M8 | P2 | new in 5.x; WP 8.3.3 (golden `edit/tools_m8/circularize`) |
| [x] | `mesh.flatten` | M8 | P2 | new in 5.x; WP 8.3.3 (golden `edit/tools_m8/flatten`) |
| [x] | `mesh.space_edge_loops_evenly` | M8 | P2 | new in 5.x; WP 8.3.3 (golden `edit/tools_m8/space_edge_loops_evenly`) |
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
| [x] | `object.posemode_toggle` | M6 | P1 | WP 6.2 (M5+M6 W0) |
| [x] | `object.mode_set_with_submode` | M8 | P2 | WP 8.3.4 |
| [x] | `object.select_all` | M1 | P0 | prereq (WP 1.19) |
| [x] | `object.select_by_type` | M1 | P1 |  |
| [x] | `object.select_pattern` | M1 | P1 |  |
| [x] | `object.select_grouped` | M8 | P2 | headless-extended (D-154): Blender's poll refuses background mode, frap runs the exec; KEYINGSET always fails (keying sets are not bound); WP 8.3.4 |
| [x] | `object.select_hierarchy` | M8 | P2 | WP 8.3.4 |
| [x] | `object.select_linked` | M8 | P2 | WP 8.3.4 |
| [x] | `object.select_random` | M8 | P2 | WP 8.3.4 |
| [x] | `object.select_camera` | M8 | P2 | WP 8.3.4 |
| [x] | `object.select_mirror` | M8 | P2 | WP 8.3.4 |
| [x] | `object.select_more` | M8 | P2 | WP 8.3.4 |
| [x] | `object.select_less` | M8 | P2 | WP 8.3.4 |
| [x] | `object.select_same_collection` | M8 | P2 | WP 8.3.4 |
| [x] | `object.camera_add` | M1 | P0 | sets scene.camera when None (virtual 3D View); object and data uniquified independently |
| [x] | `object.light_add` | M1 | P0 | 10 W (factory Light is 1000 W) |
| [x] | `object.empty_add` | M1 | P0 |  |
| [x] | `object.armature_add` | M6 | P0 | WP 6.2 (M5+M6 W0) |
| [x] | `object.add` | M8 | P2 | types with data in frap (MESH, CAMERA, LIGHT, ARMATURE, EMPTY, POINTCLOUD), the others OP_NOT_IMPLEMENTED naming the type; WP 8.3.4 |
| [x] | `object.add_named` | M8 | P2 | by name or session_uid; drop_x/drop_y OP_NOT_IMPLEMENTED (deviations §2); WP 8.3.4 |
| [x] | `object.collection_instance_add` | M8 | P2 | by name, session_uid or the collection enum; drop_x/drop_y OP_NOT_IMPLEMENTED (deviations §2); WP 8.3.4 |
| [x] | `object.delete` | M1 | P0 | prereq (WP 1.19); leaves orphan data; re-adding then names data `.001` (T0) |
| [x] | `object.duplicate` | M1 | P0 | materials shared (factory prefs) |
| [x] | `object.duplicate_move` | M1 | P0 | macro |
| [x] | `object.duplicate_move_linked` | M1 | P1 | macro |
| [x] | `object.join` | M1 | P0 | CANCELLED with WARNING reports |
| [x] | `object.convert` | M4 | P1 | apply modifiers to mesh |
| [x] | `object.join_shapes` | M6 | P2 | WP 6.3 (M5+M6 W0) |
| [x] | `object.duplicates_make_real` | M8 | P2 | collection instances (nested ones included); vertex/face instancing OP_NOT_IMPLEMENTED; WP 8.3.4 |
| [x] | `object.parent_set` | M1 | P0 | OBJECT/KEEP types M1; ARMATURE, ARMATURE_NAME, ARMATURE_ENVELOPE, BONE, BONE_RELATIVE M6 (WP 6.5); ARMATURE_AUTO (bone heat) and SUBSURF limit-surface weights M8 (WP 8.5) |
| [x] | `object.parent_clear` | M1 | P0 |  |
| [x] | `object.parent_no_inverse_set` | M1 | P1 |  |
| [x] | `object.parent_inverse_apply` | M8 | P2 | WP 8.3.5 |
| [x] | `object.origin_set` | M1 | P0 | all types |
| [x] | `object.transform_apply` | M1 | P0 | multi-user error as Blender |
| [x] | `object.location_clear` | M1 | P0 |  |
| [x] | `object.rotation_clear` | M1 | P0 |  |
| [x] | `object.scale_clear` | M1 | P0 |  |
| [x] | `object.origin_clear` | M8 | P2 | WP 8.3.5 |
| [x] | `object.visual_transform_apply` | M8 | P2 | WP 8.3.5 |
| [x] | `object.transforms_to_deltas` | M8 | P2 | WP 8.3.5 |
| [x] | `object.randomize_transform` | M8 | P2 | CPython `random` per seed; WP 8.3.5 |
| [x] | `object.align` | M8 | P2 | WP 8.3.5 |
| [x] | `object.shade_smooth` | M1 | P0 | removes sharp_face |
| [x] | `object.shade_flat` | M1 | P0 |  |
| [x] | `object.shade_smooth_by_angle` | M1 | P0 | destructive, writes sharp_edge |
| [x] | `object.shade_auto_smooth` | M4 | P1 | adds "Smooth by Angle" NODES modifier; links the group from a `Library` (2 `bpy.data.libraries` entries, as Blender) |
| [x] | `object.modifier_add` | M4 | P0 | dynamic enum by object type |
| [x] | `object.modifier_remove` | M4 | P0 |  |
| [x] | `object.modifier_apply` | M4 | P0 |  |
| [x] | `object.modifier_move_up` | M4 | P0 |  |
| [x] | `object.modifier_move_down` | M4 | P0 |  |
| [x] | `object.modifier_move_to_index` | M4 | P0 |  |
| [x] | `object.modifier_copy` | M4 | P1 |  |
| [x] | `object.modifier_set_active` | M4 | P1 |  |
| [x] | `object.modifiers_clear` | M4 | P1 |  |
| [x] | `object.subdivision_set` | M4 | P1 |  |
| [x] | `object.modifier_apply_as_shapekey` | M6 | P2 | WP 6.3 (M5+M6 W0) |
| [x] | `object.modifier_copy_to_selected` | M8 | P2 | persistent_uid D-47; particle systems OUT; WP 8.3.6 |
| [x] | `object.modifiers_copy_to_selected` | M8 | P2 | Hook/Collision in the stack: RuntimeError, rolled back D-158; WP 8.3.6 |
| [x] | `object.material_slot_add` | M1 | P0 |  |
| [x] | `object.material_slot_remove` | M1 | P0 |  |
| [x] | `object.material_slot_remove_unused` | M1 | P1 |  |
| [x] | `object.material_slot_remove_all` | M1 | P2 |  |
| [x] | `object.material_slot_assign` | M2 | P0 | edit-mode face assignment (object mode: no-op as Blender); WP 2.9 (M2 W0 assignment) |
| [x] | `object.material_slot_select` | M2 | P1 | edit mode; WP 2.4 (M2 W0 assignment) |
| [x] | `object.material_slot_deselect` | M2 | P1 | edit mode; WP 2.4 (M2 W0 assignment) |
| [x] | `object.material_slot_move` | M3 | P2 | WP 3.1 (M3+M4 W0 assignment) |
| [x] | `object.material_slot_copy` | M8 | P2 | WP 8.3.6 |
| [x] | `object.vertex_group_add` | M6 | P0 | WP 6.3 (M5+M6 W0) |
| [x] | `object.vertex_group_remove` | M6 | P0 | WP 6.3 (M5+M6 W0) |
| [x] | `object.vertex_group_assign` | M6 | P0 | edit mode; WP 6.3 (M5+M6 W0) |
| [x] | `object.vertex_group_remove_from` | M6 | P0 | edit mode; WP 6.3 (M5+M6 W0) |
| [x] | `object.vertex_group_normalize_all` | M6 | P0 | WP 6.3 (M5+M6 W0) |
| [x] | `object.vertex_group_limit_total` | M6 | P0 | glTF 4-influence limit; WP 6.3 (M5+M6 W0) |
| [x] | `object.vertex_group_assign_new` | M6 | P1 | WP 6.3 (M5+M6 W0) |
| [x] | `object.vertex_group_select` | M6 | P1 | WP 6.3 (M5+M6 W0) |
| [x] | `object.vertex_group_deselect` | M6 | P1 | WP 6.3 (M5+M6 W0) |
| [x] | `object.vertex_group_set_active` | M6 | P1 | dynamic enum; WP 6.3 (M5+M6 W0) |
| [x] | `object.vertex_group_normalize` | M6 | P1 | WP 6.3 (M5+M6 W0) |
| [x] | `object.vertex_group_clean` | M6 | P1 | WP 6.3 (M5+M6 W0) |
| [x] | `object.vertex_group_copy` | M8 | P2 | WP 8.3.7 |
| [x] | `object.vertex_group_copy_to_selected` | M8 | P2 | WP 8.3.7 |
| [x] | `object.vertex_group_invert` | M8 | P2 | WP 8.3.7 |
| [x] | `object.vertex_group_quantize` | M8 | P2 | WP 8.3.7 |
| [x] | `object.vertex_group_levels` | M8 | P2 | WP 8.3.7 |
| [x] | `object.vertex_group_lock` | M8 | P2 | WP 8.3.7 |
| [x] | `object.vertex_group_mirror` | M8 | P2 | WP 8.3.7 |
| [x] | `object.vertex_group_move` | M8 | P2 | WP 8.3.7 |
| [x] | `object.vertex_group_smooth` | M8 | P2 | WP 8.3.7 |
| [x] | `object.vertex_group_sort` | M8 | P2 | WP 8.3.7 |
| [x] | `object.vertex_weight_copy` | M8 | P2 | WP 8.3.7 |
| [x] | `object.vertex_weight_delete` | M8 | P2 | WP 8.3.7 |
| [x] | `object.vertex_weight_normalize_active_vertex` | M8 | P2 | WP 8.3.7 |
| [x] | `object.vertex_weight_paste` | M8 | P2 | WP 8.3.7 |
| [x] | `object.vertex_weight_set_active` | M8 | P2 | WP 8.3.7 |
| [x] | `object.shape_key_add` | M6 | P0 | WP 6.3 (M5+M6 W0) |
| [x] | `object.shape_key_remove` | M6 | P0 | WP 6.3 (M5+M6 W0) |
| [x] | `object.shape_key_clear` | M6 | P1 | WP 6.3 (M5+M6 W0) |
| [x] | `object.shape_key_move` | M6 | P1 | WP 6.3 (M5+M6 W0) |
| [x] | `object.shape_key_mirror` | M8 | P2 | WP 8.3.7 |
| [x] | `object.shape_key_copy` | M8 | P2 | WP 8.3.7 |
| [x] | `object.shape_key_lock` | M8 | P2 | WP 8.3.7 |
| [x] | `object.shape_key_make_basis` | M8 | P2 | WP 8.3.7 |
| [x] | `object.shape_key_apply_to_basis` | M8 | P2 | WP 8.3.7 |
| [x] | `object.shape_key_transfer` | M8 | P2 | WP 8.3.7 |
| [x] | `object.move_to_collection` | M1 | P1 |  |
| [x] | `object.link_to_collection` | M1 | P2 |  |
| [x] | `object.collection_add` | M1 | P2 |  |
| [x] | `object.collection_remove` | M1 | P2 |  |
| [x] | `object.collection_link` | M1 | P2 | dynamic enum |
| [x] | `object.collection_unlink` | M1 | P2 |  |
| [x] | `object.hide_view_set` | M3 | P1 | virtual 3D View poll; WP 3.10 (M3+M4 W0 assignment) |
| [x] | `object.hide_view_clear` | M3 | P1 | WP 3.10 (M3+M4 W0 assignment) |
| [x] | `object.make_single_user` | M3 | P1 | WP 3.10 (M3+M4 W0 assignment) |
| [x] | `object.make_links_data` | M3 | P1 | WP 3.10 (M3+M4 W0 assignment) |
| [x] | `object.hide_render_clear_all` | M8 | P2 | WP 8.3.4 |
| [ ] | `object.track_set` | M9 | P2 | post-M8 (M8 W0, M8W0-3): adds a Track To / Damped Track / Locked Track constraint, and constraints are OUT (rule 3) |
| [ ] | `object.track_clear` | M9 | P2 | post-M8 (M8 W0, M8W0-3): removes tracking constraints (constraints OUT) |
| [x] | `object.hide_collection` | M8 | P2 | WP 8.3.4 |
| [x] | `object.collection_objects_select` | M8 | P2 | headless-extended (D-154): runs on the active collection, as from the Outliner; WP 8.3.4 |
| [x] | `object.data_transfer` | M8 | P2 | T2 (the DATA_TRANSFER transfer, with `use_create`, `use_reverse_transfer`, `use_auto_transform`, `islands_precision`); the static layer items only; WP 8.3.13 |
| [x] | `object.datalayout_transfer` | M8 | P2 | T2; the objects after the source in the selection get nothing, as in Blender; WP 8.3.13 |
| [x] | `object.join_uvs` | M8 | P2 | WP 8.3.6 |
| [x] | `object.make_links_scene` | M8 | P2 | WP 8.3.6 |
| [x] | `object.instance_offset_from_cursor` | M8 | P2 | WP 8.3.5 |
| [x] | `object.instance_offset_from_object` | M8 | P2 | WP 8.3.5 |
| [x] | `object.instance_offset_to_cursor` | M8 | P2 | WP 8.3.5 |
| [x] | `object.copy_global_transform` | M8 | P2 | WP 8.3.5; headless: no clipboard (as `-b`), the text in `result.clipboard` |
| [x] | `object.copy_relative_transform` | M8 | P2 | WP 8.3.5; headless: no clipboard (as `-b`), the text in `result.clipboard` |
| [x] | `object.paste_transform` | M8 | P2 | WP 8.3.5; headless: the clipboard is always empty (as `-b`): `Clipboard does not contain a matrix` |
| [x] | `object.unlink_data` | M8 | P2 | always fails from scripts, as Blender (needs the template_ID button); WP 8.3.6 |
| [x] | `object.update_shapes` | M8 | P2 | WP 8.3.7 |
| [x] | `object.visual_geometry_to_objects` | M8 | P2 | meshes, point clouds, collection-instancing empties (no geometry-nodes instances, deviations section 2); WP 8.3.6 |
| [ ] | `object.voxel_remesh` | M9 | P2 | post-M8 (M8 W0, M8W0-3): OpenVDB level-set remesher (`BKE_mesh_remesh_voxel`), no pure-Rust port planned in M8 |
| [x] | `object.modifier_convert` | M8 | P2 | poll as Blender; particle systems OUT (OP_NOT_IMPLEMENTED); WP 8.3.6 |
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
| [x] | `transform.shear` | M8 | P2 | WP 8.3.5. Partial: Object Mode and mesh Edit Mode; armature Edit and Pose Mode answer OP_NOT_IMPLEMENTED (deviations §2, M9) |
| [x] | `transform.bend` | M8 | P2 | WP 8.3.5; Blender has no exec: a script call changes nothing (PASS_THROUGH) |
| [x] | `transform.rotate_normal` | M8 | P2 | WP 8.3.5 |
| [x] | `transform.create_orientation` | M8 | P2 | WP 8.3.5. Partial: from bones (armature Edit and Pose Mode) OP_NOT_IMPLEMENTED (deviations §2, M9) |
| [x] | `transform.select_orientation` | M8 | P2 | WP 8.3.5 |
| [x] | `transform.delete_orientation` | M8 | P2 | WP 8.3.5 |
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
| [x] | `view3d.camera_to_view_selected` | M3 | P0 | frames selected objects with scene.camera (WP 3.4) |
| [x] | `view3d.object_as_camera` | M3 | P1 | WP 3.4 (M3+M4 W0 assignment) |

### `uv.*`

| ✓ | Operator | M | Prio | Notes |
|---|---|---|---|---|
| [x] | `uv.unwrap` | M3 | P0 | CONFORMAL (LSCM) T2; ANGLE_BASED (ABF++, faer sparse LU for Eigen's) and MINIMUM_STRETCH (SLIM) since M8 (WP 8.1, goldens `uv/abf/*`, `uv/slim/*`; D-108/D-109 closed); one ABF++ island-rotation gap in deviations §2 |
| [x] | `uv.smart_project` | M3 | P0 |  |
| [x] | `uv.pack_islands` | M3 | P0 | T3 invariants |
| [x] | `uv.cube_project` | M3 | P1 |  |
| [x] | `uv.cylinder_project` | M3 | P1 | view directions use the virtual 3D View (D-10) |
| [x] | `uv.sphere_project` | M3 | P1 |  |
| [x] | `uv.reset` | M3 | P1 |  |
| [x] | `uv.average_islands_scale` | M3 | P1 |  |
| [x] | `uv.seams_from_islands` | M3 | P1 |  |
| [x] | `uv.mark_seam` | M3 | P1 |  |
| [x] | `uv.follow_active_quads` | M3 | P2 | needs active face (`mesh.loop_select(face_index=…)`) |
| [x] | `uv.lightmap_pack` | M3 | P2 | upper-case prop names are real |
| [x] | `uv.pin` | M3 | P2 |  |
| [x] | `uv.project_from_view` | M3 | P2 | frap-extended: projects from scene.camera |
| [x] | `uv.minimize_stretch` | M8 | P2 | iterations > 0 headless (`iterations=0` is the modal path, a TypeError hint); WP 8.1 (goldens `uv/minimize_stretch/*`) |
| [x] | `uv.select_all` | M8 | P2 | meaningful with sync select off; WP 8.3.8 |
| [x] | `uv.select_mode` | M8 | P2 | headless-extended: runs as in an Image Editor in UV mode (D-111); WP 8.3.8 |
| [x] | `uv.select_linked` | M8 | P2 | WP 8.3.8 |
| [x] | `uv.select_more` | M8 | P2 | headless-extended: runs as in an Image Editor in UV mode (D-111); WP 8.3.8 |
| [x] | `uv.select_less` | M8 | P2 | headless-extended: runs as in an Image Editor in UV mode (D-111); WP 8.3.8 |
| [x] | `uv.select_split` | M8 | P2 | WP 8.3.8 |
| [x] | `uv.select_overlap` | M8 | P2 | WP 8.3.8 |
| [x] | `uv.select_similar` | M8 | P2 | headless-extended: runs as in an Image Editor in UV mode (D-111); WP 8.3.8 |
| [x] | `uv.select_pinned` | M8 | P2 | WP 8.3.8 |
| [x] | `uv.select_by_winding` | M8 | P2 | WP 8.3.8 |
| [x] | `uv.shortest_path_select` | M8 | P2 | headless-extended: runs as in an Image Editor in UV mode (D-111); WP 8.3.8 |
| [x] | `uv.hide` | M8 | P2 | WP 8.3.8 |
| [x] | `uv.reveal` | M8 | P2 | WP 8.3.8 |
| [x] | `uv.weld` | M8 | P2 | WP 8.3.9 (golden `uv/edit_m8/weld_align`) |
| [x] | `uv.remove_doubles` | M8 | P2 | 2D kd-tree clusters as Blender's; WP 8.3.9 (golden `uv/edit_m8/remove_doubles`) |
| [x] | `uv.stitch` | M8 | P2 | headless-extended: runs as in an Image Editor (`blender -b` crashes without one); `selection` argument with `name`, `face_index`, `element_index`; WP 8.3.9 (goldens `uv/edit_m8/stitch*`) |
| [x] | `uv.align` | M8 | P2 | straighten reads the pin layer (Blender reads past it without one); WP 8.3.9 (golden `uv/edit_m8/weld_align`) |
| [x] | `uv.align_rotation` | M8 | P2 | Python operator ported; `correct_aspect` sees no image; WP 8.3.9 (golden `uv/edit_m8/align_rotation`) |
| [x] | `uv.arrange_islands` | M8 | P2 | libc++ `stable_sort` order; `initial_position='CURSOR'` uses the virtual Image Editor's cursor; WP 8.3.9 (goldens `uv/edit_m8/arrange`, `snap`) |
| [x] | `uv.randomize_uv_transform` | M8 | P2 | Python operator ported with CPython's Mersenne Twister; WP 8.3.9 (golden `uv/edit_m8/randomize`) |
| [x] | `uv.copy_mirrored_faces` | M8 | P2 | WP 8.3.9 (golden `uv/edit_m8/copy_mirrored_faces`) |
| [x] | `uv.snap_selected` | M8 | P2 | headless-extended: runs as in an Image Editor in UV mode without an image, its 2D cursor kept per scene; WP 8.3.9 (golden `uv/edit_m8/snap`) |
| [x] | `uv.snap_cursor` | M8 | P2 | headless-extended: runs as in an Image Editor in UV mode without an image, its 2D cursor kept per scene; WP 8.3.9 (golden `uv/edit_m8/snap`) |
| [x] | `uv.move_on_axis` | M8 | P2 | headless-extended: runs as in an Image Editor in UV mode without an image, its 2D cursor kept per scene; WP 8.3.9 (golden `uv/edit_m8/snap`) |
| [x] | `uv.rip` | M8 | P2 | `location` in UV space; WP 8.3.9 (goldens `uv/edit_m8/rip*`) |
| [x] | `uv.rip_move` | M8 | P2 | headless-extended: the translate step moves the UVs that stay selected, as in an Image Editor; WP 8.3.9 (goldens `uv/edit_m8/rip*`) |
| [x] | `uv.export_layout` | M8 | P2 | add-on operator; SVG/EPS byte-equal except the generator line, PNG drawn on the CPU (SSIM vs Blender); WP 8.3.9 (golden `uv/edit_m8/export_layout`, unit test `export_layout::tests`) |
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
| [x] | `anim.keyframe_insert` | M6 | P0 | keying-set operator; scripts usually use bpy_struct.keyframe_insert; WP 6.4 (M5+M6 W0) |
| [x] | `anim.keyframe_insert_by_name` | M6 | P1 | WP 6.4 (M5+M6 W0) |
| [x] | `anim.keyframe_delete` | M6 | P1 | WP 6.4 (M5+M6 W0) |
| [x] | `anim.keyframe_delete_v3d` | M6 | P1 | WP 6.4 (M5+M6 W0) |
| [x] | `anim.keyframe_clear_v3d` | M6 | P1 | WP 6.4 (M5+M6 W0) |
| [x] | `anim.keyframe_delete_by_name` | M6 | P2 | WP 6.4 (M5+M6 W0) |
| [ ] | `anim.keying_set_active_set` | M9 | P2 | post-M8 (M8 W0, M8W0-3): keying sets are not bound (`Scene.keying_sets`, `keying_sets.new()` raise `RNA_NOT_IMPLEMENTED`) |
| [x] | `anim.channels_bake` | M8 | P2 | headless-extended: the factory Graph Editor's channel list (D-163); T1; WP 8.3.11 |

### `screen.* (frames)`

| ✓ | Operator | M | Prio | Notes |
|---|---|---|---|---|
| [x] | `screen.frame_jump` | M6 | P1 | WP 6.4 (M5+M6 W0) |
| [x] | `screen.frame_offset` | M6 | P1 | WP 6.4 (M5+M6 W0) |
| [x] | `screen.keyframe_jump` | M6 | P2 | WP 6.4 (M5+M6 W0) |

### `armature.* (edit bones)`

| ✓ | Operator | M | Prio | Notes |
|---|---|---|---|---|
| [x] | `armature.bone_primitive_add` | M6 | P1 | WP 6.2 (M5+M6 W0) |
| [x] | `armature.extrude` | M6 | P1 | WP 6.2 (M5+M6 W0) |
| [x] | `armature.extrude_move` | M6 | P1 | macro; WP 6.2 (M5+M6 W0) |
| [x] | `armature.duplicate` | M6 | P1 | WP 6.2 (M5+M6 W0) |
| [x] | `armature.duplicate_move` | M6 | P1 | macro; WP 6.2 (M5+M6 W0) |
| [x] | `armature.delete` | M6 | P1 | WP 6.2 (M5+M6 W0) |
| [x] | `armature.subdivide` | M6 | P1 | WP 6.2 (M5+M6 W0) |
| [x] | `armature.parent_set` | M6 | P1 | WP 6.2 (M5+M6 W0) |
| [x] | `armature.parent_clear` | M6 | P1 | WP 6.2 (M5+M6 W0) |
| [x] | `armature.select_all` | M6 | P1 | WP 6.2 (M5+M6 W0) |
| [x] | `armature.calculate_roll` | M6 | P1 | WP 6.2 (M5+M6 W0) |
| [x] | `armature.roll_clear` | M6 | P1 | WP 6.2 (M5+M6 W0) |
| [x] | `armature.switch_direction` | M6 | P1 | WP 6.2 (M5+M6 W0) |
| [x] | `armature.symmetrize` | M6 | P1 | WP 6.2 (M5+M6 W0) |
| [x] | `armature.extrude_forked` | M8 | P2 | macro; the extrude step is forced `forked=True`; WP 8.3.10 |
| [x] | `armature.dissolve` | M8 | P2 | WP 8.3.10 |
| [x] | `armature.split` | M8 | P2 | WP 8.3.10 |
| [x] | `armature.separate` | M8 | P2 | new object as Blender (`add_duplicate`: object, data and actions copied, base selection kept); WP 8.3.10 |
| [x] | `armature.fill` | M8 | P2 | WP 8.3.10 |
| [x] | `armature.align` | M8 | P2 | WP 8.3.10 |
| [x] | `armature.select_more` | M8 | P2 | WP 8.3.10 |
| [x] | `armature.select_less` | M8 | P2 | WP 8.3.10 |
| [x] | `armature.select_hierarchy` | M8 | P2 | WP 8.3.10 |
| [x] | `armature.select_linked` | M8 | P2 | WP 8.3.10 |
| [x] | `armature.select_mirror` | M8 | P2 | WP 8.3.10 |
| [x] | `armature.select_similar` | M8 | P2 | `COLOR`: bone colours are not modelled, every bone matches; WP 8.3.10 |
| [x] | `armature.hide` | M8 | P2 | WP 8.3.10 |
| [x] | `armature.reveal` | M8 | P2 | WP 8.3.10 |
| [x] | `armature.flip_names` | M8 | P2 | WP 8.3.10 |
| [x] | `armature.autoside_names` | M8 | P2 | WP 8.3.10 |
| [x] | `armature.duplicate_rename` | M8 | P2 | WP 8.3.10 |
| [x] | `armature.collection_add` | M8 | P2 | bone collections; WP 8.3.10 |
| [x] | `armature.collection_remove` | M8 | P2 | WP 8.3.10 |
| [x] | `armature.collection_assign` | M8 | P2 | WP 8.3.10 |
| [x] | `armature.collection_unassign` | M8 | P2 | WP 8.3.10 |
| [x] | `armature.assign_to_collection` | M8 | P2 | a failed call keeps no new collection (D-157); WP 8.3.10 |
| [x] | `armature.move_to_collection` | M8 | P2 | a failed call keeps no new collection (D-157); WP 8.3.10 |
| — | `armature.click_extrude` | OUT | — | interactive |
| — | `armature.select_linked_pick` | OUT | — |  |
| — | `armature.shortest_path_pick` | OUT | — |  |

### `pose.*`

| ✓ | Operator | M | Prio | Notes |
|---|---|---|---|---|
| [x] | `pose.select_all` | M6 | P1 | WP 6.2 (M5+M6 W0) |
| [x] | `pose.transforms_clear` | M6 | P1 | WP 6.2 (M5+M6 W0) |
| [x] | `pose.loc_clear` | M6 | P1 | WP 6.2 (M5+M6 W0) |
| [x] | `pose.rot_clear` | M6 | P1 | WP 6.2 (M5+M6 W0) |
| [x] | `pose.scale_clear` | M6 | P1 | WP 6.2 (M5+M6 W0) |
| [x] | `pose.armature_apply` | M6 | P1 | apply pose as rest pose; WP 6.2 (M5+M6 W0) |
| [x] | `pose.rotation_mode_set` | M6 | P1 | WP 6.2 (M5+M6 W0) |
| [x] | `pose.user_transforms_clear` | M8 | P2 | the action evaluated at the current frame on a pose copy, else the rest pose; WP 8.3.11 |
| [x] | `pose.visual_transform_apply` | M8 | P2 | T1; WP 8.3.11 |
| [x] | `pose.copy` | M8 | P2 | copy buffer in the frap process (D-161); WP 8.3.11 |
| [x] | `pose.paste` | M8 | P2 | `flipped`, `selected_mask`; rotation converted to the target mode; WP 8.3.11 |
| [x] | `pose.select_hierarchy` | M8 | P2 | WP 8.3.11 |
| [x] | `pose.select_parent` | M8 | P2 | WP 8.3.11 |
| [x] | `pose.select_linked` | M8 | P2 | WP 8.3.11 |
| [x] | `pose.select_mirror` | M8 | P2 | WP 8.3.11 |
| [x] | `pose.select_grouped` | M8 | P2 | COLOR matches every bone (no bone colours), KEYINGSET fails (no keying sets); WP 8.3.11 |
| [x] | `pose.hide` | M8 | P2 | WP 8.3.11 |
| [x] | `pose.reveal` | M8 | P2 | WP 8.3.11 |
| [x] | `pose.flip_names` | M8 | P2 | WP 8.3.11 |
| [x] | `pose.autoside_names` | M8 | P2 | WP 8.3.11 |
| [x] | `pose.quaternions_flip` | M8 | P2 | WP 8.3.11 |
| — | `pose.constraint_add` | OUT | — | constraints out of scope |
| — | `pose.ik_add` | OUT | — |  |
| — | `pose.breakdown` | OUT | — | interactive pose tools |
| — | `pose.push` | OUT | — |  |
| — | `pose.relax` | OUT | — |  |

### `nla.*`

| ✓ | Operator | M | Prio | Notes |
|---|---|---|---|---|
| [x] | `nla.action_pushdown` | M6 | P1 | headless-extended: active object's AnimData; feeds NLA_TRACKS export; WP 6.4 (M5+M6 W0) |
| [x] | `nla.tracks_add` | M6 | P2 | headless-extended; WP 6.4 (M5+M6 W0) |
| [x] | `nla.tracks_delete` | M8 | P2 | headless-extended: active object's AnimData; WP 8.3.11 |
| [x] | `nla.actionclip_add` | M8 | P2 | headless-extended: active object's AnimData; WP 8.3.11 |

### `render.*`

| ✓ | Operator | M | Prio | Notes |
|---|---|---|---|---|
| [x] | `render.render` | M1 | P0 | still M1 (Workbench); EEVEE M5; animation=True M6; output formats: every image format but JPEG2000 and AVIF, multilayer OpenEXR; video (FFMPEG) M8 (decision log M56S-2); `scene=` / `layer=` other than the context ones and `use_sequencer_scene=True`: `OP_NOT_IMPLEMENTED` until M8 (deviations §2) |
| [x] | `render.opengl` | M8 | P2 | headless-extended D-160: Workbench from the scene camera; WP 8.3.6 |

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
| [x] | `scene.new` | M3 | P1 | WP 3.10; multi-scene glTF |
| [x] | `scene.delete` | M3 | P2 | WP 3.10 |
| [x] | `scene.gltf2_variant_add` | M8 | P2 | glTF add-on (`KHR_materials_variants_ui` on, else unknown as in Blender); `VariantName` made unique like the add-on's name update; M8 W0 (golden `gltf/variants_ops/scene_ops`) |
| [x] | `scene.gltf2_variant_remove` | M8 | P2 | glTF add-on; removes the active variant of `bpy.data.scenes[0]` and the mesh mappings that use it, with the add-on's in-place index removal; M8 W0 |
| [x] | `scene.gltf2_display_variant` | M8 | P2 | glTF add-on; every mesh object's slots take the materials mapped to `bpy.data.scenes[0].gltf2_active_variant`; M8 W0 |
| [x] | `scene.gltf2_reset_to_original` | M8 | P2 | glTF add-on; the active object's slots take `gltf2_variant_default_materials`; M8 W0 |
| [x] | `scene.gltf2_assign_to_variant` | M8 | P2 | glTF add-on; records the active object's slot materials for the active variant; M8 W0 |
| [x] | `scene.gltf2_assign_as_original` | M8 | P2 | glTF add-on; records the active object's slot materials as defaults; M8 W0 |
| [x] | `scene.gltf2_variants_slot_add` | M8 | P2 | glTF add-on; adds the active variant to the active slot's mapping; M8 W0 |
| [x] | `scene.gltf2_material_to_variant` | M8 | P2 | glTF add-on; re-points the active mapping entry to `Mesh.gltf2_variant_pointer`'s variant (CANCELLED without a match, as the add-on); M8 W0 |
| [x] | `scene.gltf2_remove_material_variant` | M8 | P2 | glTF add-on; removes the active mapping entry (CANCELLED without a mapping for the active slot); M8 W0 |
| [ ] | `scene.gltf2_action_filter_refresh` | M9 | P2 | glTF add-on, post-M8 with `export_action_filter` (deviations §2, M8W0-3) |
| — | `scene.view_layer_add` | OUT | — |  |
| — | `scene.view_layer_remove` | OUT | — |  |

### `material.*`

| ✓ | Operator | M | Prio | Notes |
|---|---|---|---|---|
| [x] | `material.new` | M1 | P0 | does not assign to a slot from scripts (as Blender) |
| [x] | `material.copy` | M8 | P2 | headless-extended D-159: active material; session copy buffer outside undo; WP 8.3.6 |
| [x] | `material.paste` | M8 | P2 | headless-extended D-159: active material; WP 8.3.6 |

### `image.*`

| ✓ | Operator | M | Prio | Notes |
|---|---|---|---|---|
| [x] | `image.new` | M3 | P0 | no users, shown nowhere (as Blender from scripts); `COLOR_GRID` without its labels; WP 3.2 |
| [x] | `image.open` | M3 | P0 | `files`/`directory` with sequence detection; no UDIM detection; WP 3.2 |
| [x] | `image.pack` | M3 | P1 | poll fails headless as in Blender (no Image Editor / `edit_image`); use `bpy.data.images[...].pack()`; WP 3.2 |
| [x] | `image.unpack` | M3 | P2 | poll fails headless as in Blender (no Image Editor / `edit_image`); use `bpy.data.images[...].unpack(method=…)`; WP 3.2 |
| [x] | `image.reload` | M3 | P2 | `CANCELLED` headless as in Blender (no context image); use `bpy.data.images[...].reload()`; WP 3.2 |
| [x] | `image.save` | M3 | P2 | poll fails headless as in Blender (no Image Editor / `edit_image`); use `bpy.data.images[...].save()`; WP 3.2 |
| [x] | `image.save_as` | M3 | P2 | poll fails headless as in Blender (no Image Editor / `edit_image`); use `bpy.data.images[...].save(filepath=…)` / `.save_render(…)`; WP 3.2 |

### `world.*`

| ✓ | Operator | M | Prio | Notes |
|---|---|---|---|---|
| [x] | `world.new` | M3 | P1 |  |

### `collection.*`

| ✓ | Operator | M | Prio | Notes |
|---|---|---|---|---|
| [x] | `collection.create` | M8 | P2 | WP 8.3.6 |
| [x] | `collection.objects_add_active` | M8 | P2 | WP 8.3.6 |
| [x] | `collection.objects_remove` | M8 | P2 | WP 8.3.6 |
| [x] | `collection.objects_remove_all` | M8 | P2 | WP 8.3.6 |
| [x] | `collection.objects_remove_active` | M8 | P2 | WP 8.3.6 |

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
| [x] | `ed.undo` | M8 | P2 | headless-extended (D-149): an interactive Blender's undo stack (`blender -b` has none before `ed.undo_push()`), steps for `UNDO` operators, RNA writes and data-changing RNA calls; Memfile / Edit Mesh / Edit Armature steps sharing the state's data; WP 8.4 (goldens `undo/*`) |
| [x] | `ed.redo` | M8 | P2 | headless-extended (D-149); WP 8.4 |
| [x] | `ed.undo_redo` | M8 | P2 | headless-extended (D-149): repeats the last `REGISTER`+`UNDO` operator call (`object.mode_set` registers its toggle operator); WP 8.4 |
| [x] | `ed.undo_push` | M8 | P2 | headless-extended (D-149): tags `bpy.data.is_dirty` as `ED_undo_push` does; the implicit steps (operators, RNA writes and calls) leave `is_dirty` as `blender -b` does; WP 8.4 |

### `marker.* (timeline markers)`

| ✓ | Operator | M | Prio | Notes |
|---|---|---|---|---|
| [x] | `marker.add` | M7 | P2 | headless-extended: the factory Timeline's markers (the context scene's); `F_<frame>` at the current frame; WP 7.6 (golden `scene/markers/operators`) |
| [x] | `marker.camera_bind` | M7 | P2 | headless-extended; binds the active object, the scene camera follows (`BKE_scene_camera_switch_update`); WP 7.6 |
| [x] | `marker.delete` | M7 | P2 | headless-extended; WP 7.6 |
| [x] | `marker.duplicate` | M7 | P2 | headless-extended; copies at the head of the list, as Blender; WP 7.6 |
| [x] | `marker.make_links_scene` | M7 | P2 | headless-extended; `scene` lists `bpy.data.scenes`; WP 7.6 |
| [x] | `marker.move` | M7 | P2 | headless-extended; the scene camera follows; WP 7.6 |
| [x] | `marker.rename` | M7 | P2 | headless-extended; first selected marker; WP 7.6 |
| [x] | `marker.select` | M7 | P2 | headless-extended: `mouse_x` in the factory Timeline's region pixels (exact hits only headless); WP 7.6 |
| [x] | `marker.select_all` | M7 | P2 | headless-extended; WP 7.6 |
| [x] | `marker.select_box` | M7 | P2 | headless-extended: `xmin`/`xmax` in the factory Timeline's region pixels; WP 7.6 |
| [x] | `marker.select_leftright` | M7 | P2 | headless-extended; WP 7.6 |

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
| [x] | `SUBSURF` | `SubsurfModifier` | M4 | P0 | frap-subdiv; T2, layout pass towards T1 |
| [x] | `MIRROR` | `MirrorModifier` | M4 | P0 |  |
| [x] | `ARRAY` | `ArrayModifier` | M4 | P0 |  |
| [x] | `BEVEL` | `BevelModifier` | M4 | P0 | shares mesh.bevel core |
| [x] | `SOLIDIFY` | `SolidifyModifier` | M4 | P0 | simple + complex |
| [x] | `BOOLEAN` | `BooleanModifier` | M4 | P0 | MANIFOLD T2 (hole-bridge and coincident-vertex configurations T3, see deviations D-100; EXACT D-101), EXACT T3, FLOAT T2 since M8 (WP 8.6: `BM_mesh_intersect` port, deterministic cut order D-145; goldens `modifiers/boolean_float/*`) |
| [x] | `TRIANGULATE` | `TriangulateModifier` | M4 | P0 |  |
| [x] | `DECIMATE` | `DecimateModifier` | M4 | P0 | COLLAPSE/UNSUBDIV/DISSOLVE |
| [x] | `WELD` | `WeldModifier` | M4 | P0 |  |
| [x] | `DISPLACE` | `DisplaceModifier` | M4 | P0 | Texture IMAGE/NONE |
| [x] | `NODES` | `NodesModifier` | M4 | P0 | "Smooth by Angle" only; inputs at properties.inputs.Input_1.value |
| [x] | `EDGE_SPLIT` | `EdgeSplitModifier` | M4 | P1 |  |
| [x] | `WEIGHTED_NORMAL` | `WeightedNormalModifier` | M4 | P1 |  |
| [x] | `SMOOTH` | `SmoothModifier` | M4 | P1 |  |
| [x] | `CAST` | `CastModifier` | M4 | P1 |  |
| [x] | `SIMPLE_DEFORM` | `SimpleDeformModifier` | M4 | P1 |  |
| [x] | `WIREFRAME` | `WireframeModifier` | M4 | P1 |  |
| [x] | `SCREW` | `ScrewModifier` | M4 | P1 |  |
| [x] | `MASK` | `MaskModifier` | M4 | P1 |  |
| [x] | `BUILD` | `BuildModifier` | M4 | P2 |  |
| [x] | `NORMAL_EDIT` | `NormalEditModifier` | M4 | P2 |  |
| [x] | `LAPLACIANSMOOTH` | `LaplacianSmoothModifier` | M4 | P2 |  |
| [x] | `ARMATURE` | `ArmatureModifier` | M6 | P0 | LBS, envelopes, mask group, preserve volume (dual quaternions), multi modifier; armature-parent virtual modifier (WP 6.5); B-Bone segments M8 |
| [ ] | `REMESH` | `RemeshModifier` | M9 | P2 | post-M8 (M8 W0, M8W0-3): VOXEL needs OpenVDB, BLOCKS/SMOOTH/SHARP the dualcon octree contourer (`intern/dualcon`); passes through with the WARNING |
| [x] | `SHRINKWRAP` | `ShrinkwrapModifier` | M8 | P2 | every `wrap_method` (BVH nearest point/vertex/target-project, watertight ray casts, auxiliary target, culling, limit, `subsurf_levels` as Blender's uniform refinement) and `wrap_mode`; bit-exact with Blender's arm64 `invert_m4_m4`; WP 8.3.12 (goldens `modifiers/m8_deform/shrinkwrap/*`) |
| [x] | `UV_PROJECT` | `UVProjectModifier` | M8 | P2 | empties and cameras (perspective, orthographic, panoramic), several projectors chosen per face normal; `projectors[i].object` bound (saved in `.frap`, remapped on removal); WP 8.3.12 (golden `modifiers/m8_deform/uv_project/projectors`) |
| [x] | `UV_WARP` | `UVWarpModifier` | M8 | P2 | objects/bones, 2D transform, vertex group; WP 8.3.12 (goldens `modifiers/m8_deform/uv_warp/transforms`, `warp/bones`) |
| [x] | `CORRECTIVE_SMOOTH` | `CorrectiveSmoothModifier` | M8 | P2 | `rest_source='ORCO'` (the edit mesh in Edit Mode); `BIND` needs `object.correctivesmooth_bind` (OUT): "Bind data required", `is_bind` is `False`; WP 8.3.12 (goldens `modifiers/m8_deform/corrective_smooth/*`) |
| [x] | `WAVE` | `WaveModifier` | M8 | P2 | time-dependent, along normals, start object, IMAGE/NONE textures; WP 8.3.12 (goldens `modifiers/m8_deform/wave/frame_*`) |
| [x] | `WARP` | `WarpModifier` | M8 | P2 | every falloff (`CURVE`: the default curve's table, `falloff_curve` unbound), volume preserve, bones, IMAGE/NONE textures; WP 8.3.12 (goldens `modifiers/m8_deform/warp/*`) |
| [x] | `DATA_TRANSFER` | `DataTransferModifier` | M8 | P2 | T2: every data type and mapping mode, UV islands, mix modes; WP 8.3.13 |
| [x] | `VERTEX_WEIGHT_EDIT` | `VertexWeightEditModifier` | M8 | P2 | T2; `CURVE` is the default curve, mask textures are not evaluated; WP 8.3.13 |
| [x] | `VERTEX_WEIGHT_MIX` | `VertexWeightMixModifier` | M8 | P2 | T2; mask textures are not evaluated; WP 8.3.13 |
| [x] | `VERTEX_WEIGHT_PROXIMITY` | `VertexWeightProximityModifier` | M8 | P2 | T2; mask textures are not evaluated; WP 8.3.13 |
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
| [x] | `ShaderNodeBackground` | Background | M3 | world colour (render M5: WP 5.2 world probe, `Color`/`Strength`) |
| [x] | `ShaderNodeTexEnvironment` | Environment Texture | M3 | world HDRI (render M5: WP 5.2 equirectangular probe, Mapping Z rotation; other projections are a deviations §2 gap) |
| [ ] | `ShaderNodeOutputWorld` | World Output | M3 | world root |

## D. RNA structs

A struct is done when its `StructDef` is generated, its bindings pass `check_bindings`, and golden cases read/write its listed properties through `tests/golden/props.toml`. Properties not bound yet raise `NotImplementedError` naming the struct and property.

| ✓ | Struct(s) | M | Notes |
|---|---|---|---|
| [ ] | ID (name, users, use_fake_user, rename(), session_uid, library, is_linked_packed, is_embedded_data, custom properties) | M1 | names ≤255 bytes; unique per (name, library); embedded IDs via owner |
| [ ] | Library | M1 | minimal: name, filepath, parent, is_archive; created by built-in linked groups (M4 use) |
| [ ] | Window (stub: scene, view_layer) | M1 read / M3 write | `bpy.context.window.scene = …` switches scenes |
| [ ] | AddonPreferences for `io_scene_gltf2` (`KHR_materials_variants_ui`, …) | M1 data / M7 variants | via `bpy.context.preferences.addons['io_scene_gltf2'].preferences`; `KHR_materials_variants_ui` registers the variant RNA (M7, WP 7.5) |
| [ ] | BlendData + BlendData<Coll> (objects, meshes, materials, cameras, lights, collections, worlds, scenes, images, node_groups, libraries, pointclouds) | M1 | new/remove/get/find/[i]/['name']; order rule §5.4; `scenes.new` M3 |
| [ ] | Context (object, active_object, selected_objects, selected_editable_objects, visible_objects, selectable_objects, editable_objects, scene, view_layer, collection, tool_settings, mode, edit_object, objects_in_mode*, blend_data) | M1 | read-only |
| [x] | Context (preferences, space_data stub) | M2 | WP 2.3 (`frap-session/src/resolve/context/ui.rs`): `window`, `space_data` (`SpaceView3D` stub: `type`, `shading`, read-only `region_3d`), factory `preferences`; `screen`/`area`/`region` read `None` (deviations D-61, D-62) |
| [ ] | Object | M1 | transforms, parent, matrix_world (fresh), dimensions, data, type, mode, material_slots, hide_*, display; vertex_groups M6; pose M6; animation_data M6 |
| [ ] | ObjectModifiers, Modifier (+ every <Type>Modifier settings struct) | M1 data / M4 eval | generated settings |
| [ ] | Mesh, MeshVertex, MeshEdge, MeshPolygon, MeshLoop, MeshLoopTriangle | M1 | SoA proxies (WP 1.27); use_smooth/use_seam/use_edge_sharp proxies; foreach_get/foreach_set; edit-mode reads via cached view (M2) |
| [ ] | AttributeGroupMesh, Attribute and typed attributes (FloatAttribute, FloatVectorAttribute, FloatColorAttribute, ByteColorAttribute, BoolAttribute, IntAttribute, Float2Attribute, …) | M1 | attributes.new/remove, active_color, default_color |
| [ ] | MeshUVLoopLayer, MeshUVLoop, UVLoopLayers | M1 | uv_layers.new(do_init), active, active_render |
| [ ] | Material, MaterialSlot, IDMaterials (mesh.materials) | M1 | diffuse_color (Workbench), surface_render_method, use_backface_culling; MaterialSlot derived from data + per-object overrides |
| [ ] | PointCloud | M1 data / M3 glTF | POINTS export, `import_point_as_pointcloud`; RNA `points` (`co`, `radius`, `index`), `.frap` persistence (WP 3.7); not done: `attributes`/`materials` RNA and `resize()` are not bound yet |
| [ ] | ShaderNodeTree, Node, NodeSocket (+ typed sockets), NodeLink, Nodes, NodeLinks | M1 | first-enabled socket lookup; generic for all node types; material/world trees are embedded IDs |
| [ ] | Camera, CameraDOFSettings | M1 | lens/angle hook; glTF M3 |
| [ ] | Light, PointLight, SunLight, SpotLight, AreaLight | M1 | type re-typing hook; glTF M3 |
| [x] | World | M1 | node tree; HDRI render M5 (WP 5.2); unbound world settings are a deviations §2 gap |
| [ ] | Scene, RenderSettings, ImageFormatSettings, ColorManagedViewSettings, ColorManagedDisplaySettings | M1 | generated plain structs |
| [ ] | SceneDisplay, View3DShading | M1 | Workbench settings |
| [x] | SceneEEVEE | M1 data / M5 render | read by the EEVEE-like renderer (WPs 5.1–5.5: `taa_render_samples`, `filter_size`, `clamp_surface_*`, shadow and ray-tracing settings); gaps in deviations §2 |
| [ ] | ToolSettings (mesh_select_mode, transform_pivot_point, proportional_*, double_threshold, use_uv_select_sync, vertex_group_weight) | M1 data / M2 use | subset |
| [ ] | View3DCursor, UnitSettings | M1 |  |
| [ ] | ViewLayer, LayerObjects, LayerCollection | M1 | bases, active, selection per view layer |
| [ ] | Collection, CollectionObjects, CollectionChildren | M1 | link/unlink |
| [ ] | Depsgraph, DepsgraphObjectInstance (evaluated_get, to_mesh) | M1 | always fresh |
| [x] | Image, ImagePackedFile | M1 meta / M3 full | pixels, pack, save, save_render; `PackedFile.data` (bytes) not readable; WP 3.2 |
| [x] | Texture, ImageTexture | M4 | DISPLACE only; field set frozen by M3+M4 W0 (`TextureFields`), bindings and `bpy.data.textures.new` WP 4.2 |
| [x] | VertexGroup, VertexGroups, VertexGroupElement | M6 | names on Mesh, RNA on Object |
| [x] | Key, ShapeKey | M6 |  |
| [x] | Armature, Bone, EditBone, ArmatureEditBones, Pose, PoseBone, BoneCollection | M6 |  |
| [x] | Action, ActionSlot, ActionLayer, ActionStrip, ActionKeyframeStrip, ActionChannelbag, FCurve, Keyframe, FCurveKeyframePoints, ActionGroup | M6 | slotted actions; action.fcurves → 5.x hint |
| [x] | AnimData, NlaTrack, NlaTracks, NlaStrip, NlaStrips | M6 |  |
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
| [ ] | `Mesh.update(), validate(), calc_loop_triangles(), transform(matrix), clear_geometry()` | M1 | `validate()` answers `False` for a valid mesh and refuses to repair (`RNA_NOT_IMPLEMENTED`); `transform(shape_keys=True)` transforms the shape keys (WP 6.3) |
| [x] | `Mesh.set_sharp_from_angle(angle), flip_normals(), split_faces()` | M2 | moved from M1 (M1 stabilization); `object.shade_smooth_by_angle` covers sharp-from-angle meanwhile; WP 2.9 (`frap-ops-data/src/funcs/mesh_*.rs`, M2 W0 assignment) |
| [x] | `Mesh.calc_tangents(uvmap)` | M3 | bevy_mikktspace (`frap-geom/src/mikktspace.rs`); WP 3.7 (M3+M4 W0 assignment) |
| [ ] | `mesh.materials.append/pop/clear, MaterialSlot.material/link` | M1 |  |
| [ ] | `mesh.attributes.new/remove, mesh.uv_layers.new, mesh.color_attributes.new/remove` | M1 | colour attributes default to white and become active/default when none is |
| [x] | `mesh.uv_layers.remove, mesh.vertex_colors.new/remove` | M2 | moved from M1 (M1 stabilization); `attributes.remove` removes a UV map meanwhile; vertex_colors creates "Col" BYTE_COLOR CORNER; WP 2.9 (`funcs/{u_v_loop_layers_remove,loop_colors_new}.rs`, M2 W0 assignment); `vertex_colors.remove` since M8 stabilization round 6 (`funcs/loop_colors_remove.rs`, golden `rna/loop_colors_remove`) |
| [ ] | `node_tree.nodes.new/remove, node_tree.links.new/remove` | M1 | links.new replaces existing input link |
| [x] | `obj.modifiers.new(name, type)/remove/move/clear` | M4 (data from M1) | WP 4.1 (`funcs/object_modifiers_*.rs`); M4 golden prerequisite |
| [x] | `Image.pack/unpack/save/save_render/scale/reload, bpy.data.images.load` | M3 | also `update`, `buffers_free`, `filepath_from_user`, `ImagePackedFile.save`; `pack(data=…)` raw bytes not supported; `Render Result.save_render()` reports no data (render with `write_still`); WP 3.2 |
| [x] | `Scene.frame_set(frame, subframe=0.0)` | M6 | writes animated values to originals |
| [x] | `bpy_struct.keyframe_insert(data_path, *, index=-1, frame=None, group="", options=set(), keytype='KEYFRAME')` | M6 | keyword-only after data_path |
| [x] | `bpy_struct.keyframe_delete(data_path, *, index=-1, frame=None, group="")` | M6 |  |
| [x] | `action.layers/strips/channelbag(slot, ensure=False), channelbag.fcurves.new/remove, fcurve.keyframe_points.insert/add` | M6 |  |
| [x] | `AnimData: animation_data_create(), action, action_slot; nla_tracks.new(), track.strips.new(name, start, action)` | M6 |  |
| [x] | `obj.vertex_groups.new/remove, VertexGroup.add(index, weight, type)/remove/weight(index)` | M6 | works in object mode |
| [x] | `Object.shape_key_add(name, from_mix)/shape_key_remove/shape_key_clear` | M6 |  |
| [x] | `Bone/EditBone/PoseBone` `.vector/.center/.x_axis/.y_axis/.z_axis/.basename/.parent_recursive/.children/.children_recursive/.children_recursive_basename`, `.translate(vec)`, `.parent_index(bone)` (Python-only), `EditBone.align_roll(vector)`, `bpy_struct.id_data` | M6 | M5+M6 stabilization (decision log M56S-3); `EditBone/Armature.transform`, `Bone.convert_local_to_pose`, `evaluate_envelope`, `Object.convert_space` are M8 (deviations §2) |
| [ ] | `bpy_struct.path_resolve / path_from_id, repr` | M1 | byte-identical to Blender, incl. embedded roots |
| [ ] | `<collection>.foreach_get(attr[, seq]) / foreach_set(attr, seq)`, slices `coll[a:b]` | M1 | one-argument `foreach_get` is a frap extension; bulk `$array` encoding |
| [ ] | `bpy.ops.<ns>.<op>.poll() / get_rna_type() / idname() / idname_py()` | M1 | poll returns bool + `unmet` requirements |
| [x] | `bpy.data.scenes.new(name) / remove`, `context.window.scene = …` | M3 | WP 3.10 |
| — | `bpy_struct.driver_add/driver_remove` | OUT | drivers out of scope |

## F. glTF features

| ✓ | Feature | Export M | Import M | Notes |
|---|---|---|---|---|
| [ ] | Static meshes, node hierarchy (TRS), multi-material primitives, NORMAL, TEXCOORD_0 | M1 | M1 | Blender-compat rules, looptris |
| [x] | Multiple scenes (`scenes[]`, default `scene`, `use_active_scene`) | M3 | M3 | WP 3.10 |
| [x] | Non-triangle modes: LINES (`use_mesh_edges`), POINTS (`use_mesh_vertices`, point clouds); import of strips, fans, line strips/loops | M3 | M3 | WP 3.7; POINTS from loose vertices are written without `indices`, as the add-on does (M5+M6 stabilization round 3) |
| [ ] | pbrMetallicRoughness factors + baseColorTexture, doubleSided | M1 | M1 |  |
| [x] | Multiple UV sets, vertex colours (COLOR_n, fake COLOR_0), TANGENT, extras | M3 | M3 | WP 3.7 (with WP 3.5 for `export_vertex_color='MATERIAL'`); texCoord of an Attribute node not resolved (deviations §2) |
| [x] | All textures/samplers/images (PNG/JPEG passthrough, ORM packing), alpha modes OPAQUE/MASK/BLEND | M3 | M3 | WP 3.5; WebP passthrough and EXT_texture_webp since WP 7.3 |
| [x] | Cameras (perspective/orthographic) | M3 | M3 | export_cameras=False by default |
| [x] | KHR_lights_punctual | M3 | M3 | export_lights=False by default; AREA warning |
| [x] | KHR_texture_transform | M3 | M3 | marked required (Blender quirk); WP 3.5 |
| [x] | KHR_materials_emissive_strength, unlit | M3 | M3 | WP 3.5 |
| [x] | KHR_materials_transmission, clearcoat, sheen, ior, specular, volume, iridescence, anisotropy, dispersion | M3 | M3 | WP 3.6: export goldens per extension, import raise round-trip (`tests/golden/cases/gltf/materials_ext/`); grayscale anisotropy textures not baked (WARNING) |
| [x] | KHR_materials_pbrSpecularGlossiness | — | M3 | import only (archived); M3 stabilization: the add-on's graph (IOR 1000, Specular Tint, baked roughness image `<image>.001`), golden `tests/golden/cases/gltf/materials_core/spec_gloss.py`; read from the raw extension JSON (no typed `frap-gltf` struct) |
| [x] | Animations (TRS, weights; ACTIONS/ACTIVE_ACTIONS/BROADCAST/NLA_TRACKS/SCENE; LINEAR/STEP/CUBICSPLINE, sampling) | M6 | M6 | WP 6.7: every `export_animation_mode`, sampling and optimisation option, bone and shape-key channels (with WP 6.8); import of one action per animation stashed on NLA tracks, the first restored and evaluated at the current frame; goldens `tests/golden/cases/gltf/animation/` (deviations §2) |
| [x] | Skins (joints, inverse bind matrices, JOINTS_n/WEIGHTS_n) | M6 | M6 | WP 6.8: every skin option (def bones, flatten, leaf bones, rest/pose, armature removal, influences), neutral bone, unused skins, `export_apply` without ARMATURE modifiers; import with every `bone_heuristic` and `guess_original_bind_pose`; goldens `tests/golden/cases/gltf/skins/` (deviations §2) |
| [x] | Morph targets (POSITION/NORMAL deltas, sparse, default weights) | M6 | M6 | WP 6.8: POSITION/NORMAL/TANGENT targets, the sparse heuristic, weights and targetNames; import of shape keys and weights (no mesh sharing for animated weights); weights animation channels are WP 6.7's |
| [x] | KHR_animation_pointer | M6 | M6 | WP 6.9: camera, light and material channels both ways, `export_convert_animation_pointer` for TRS/weights; import of node TRS / weights pointers (M5+M6 W3); goldens `tests/golden/cases/gltf/pointer/` (gaps in deviations §2) |
| [x] | KHR_mesh_quantization | M7 | M7 | WP 7.1: `frap_mesh_quantization`, the meshopt path (as the add-on quantizes for meshopt) and the gltfpack path (`export_use_gltfpack`, `export_gltfpack_*`); import dequantizes; goldens `gltf/meshopt/` |
| [x] | EXT_meshopt_compression, KHR_meshopt_compression | M7 | M7 | WP 7.1: `export_meshopt_compression_enable`, `export_meshopt_extension` (EXT and KHR, buffer-level as the add-on's `finalize_buffer`, fallback buffer); import of Blender-written files (GLB, `GLTF_SEPARATE`); goldens `gltf/meshopt/` |
| [x] | KHR_draco_mesh_compression | M7 | M7 | WP 7.2: spike gate passed with `draco-core` (pure Rust, frap-cli default; `draco-sys` behind its feature); every `export_draco_*` prop and `frap_draco_backend`; import decodes within quantization bounds (D-132); goldens `gltf/draco/` |
| [x] | EXT_texture_webp | M7 (lossless and lossy; lossless moved from M3, decision log M34S3-5) | M3 | WP 7.3: `export_image_format='WEBP'` (lossless at quality 100, else lossy; lossless with a WARNING without the `webp-lossy` feature), `export_image_add_webp`, `export_image_webp_fallback`, WebP sources, as texture.py writes them; Blender comparison `tests/golden/cases/gltf/textures/webp_export.py` (`frap-ops-io-tests/tests/gltf_textures.rs`); re-encoded bytes differ (deviations §2) |
| [x] | KHR_texture_basisu | M7 (feature basisu-encode) | M7 | WP 7.3 (frap extension, D-133): KTX2 read (ETC1S, UASTC, Zstandard; feature `basisu`, on in frap-cli) for textures with no other image, render; `frap_texture_format='KTX2'`, `frap_basisu_mode` (encoder behind `basisu-encode`, off by default; KTX2 images copied without it), with the add-on's images kept as fallbacks |
| [x] | KHR_materials_variants | M7 | M7 | not part of M3's "every KHR material extension" (decision log M34S3-5); add-on data model (`Scene.gltf2_KHR_materials_variants_variants`, `gltf2_active_variant`, `Mesh.gltf2_variant_mesh_data` / `_default_materials` / `_pointer`, ID-property collections with `add/remove/clear/move`); RNA exists only with `KHR_materials_variants_ui` on (D-134); import turns it on; `variants_reset_to_original` on export; goldens `gltf/variants/`; WP 7.5 |
| [x] | EXT_mesh_gpu_instancing | M7 (`export_gpu_instances`, Blender prop) | M7 | children of a node sharing a mesh (add-on rule, holders `<name>.<k>`), channel quirk D-139; import: one object per instance; goldens `gltf/instancing/`; WP 7.5 |
| [x] | KHR_node_visibility, KHR_materials_diffuse_transmission | M7 (frap_*) | M7 | not produced by Blender: `frap_node_visibility` (`hide_render`), `frap_diffuse_transmission` (thin-walled subsurface; imported back, D-130); with `frap_compat='SPEC'`, `frap_interleave` and UNSIGNED_BYTE indices; WP 7.4 |

## G. Totals

Recounted at M8 W0 with the codegen parser (`xtask/src/codegen/model.rs::parse_operators_md`: section A rows whose `M` cell is `M<n>` or `OUT`).

| Milestone | Operators |
|---|---|
| M1 | 56 |
| M2 | 125 |
| M3 | 29 |
| M4 | 15 |
| M6 | 54 |
| M7 | 11 |
| M8 | 192 (183 for the M8 WPs, 9 glTF variant operators done in M8 W0) |
| M9 (post-M8) | 5 |
| OUT | 60 |
| total listed | 547 |
