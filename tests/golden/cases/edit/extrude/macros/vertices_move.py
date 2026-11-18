# PENDING (WP 2.5 -> WP 2.12): enable by renaming to `.py` once `transform.translate` and
# `transform.shrink_fatten` work in Edit Mode (TRANSFORM_MODE_HANDLERS entry of WP 2.12); the
# Blender fixture is already blessed (tests/golden/fixtures/edit/extrude/macros/).
# WP 2.5: mesh.extrude_vertices_move on one vertex (V9 E13 F6), twice (a wire chain).
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[6])
bpy.ops.mesh.extrude_vertices_move(TRANSFORM_OT_translate={"value": (0, 0, 1)})
bpy.ops.mesh.extrude_vertices_move(TRANSFORM_OT_translate={"value": (0, 1, 0)})
bpy.data.meshes['Cube'].total_vert_sel
bpy.data.meshes['Cube'].total_face_sel
