# PENDING (WP 2.5 -> WP 2.12): enable by renaming to `.py` once `transform.translate` and
# `transform.shrink_fatten` work in Edit Mode (TRANSFORM_MODE_HANDLERS entry of WP 2.12); the
# Blender fixture is already blessed (tests/golden/fixtures/edit/extrude/macros/).
# WP 2.5: mesh.extrude_edges_move on one edge in edge mode (V10 E15 F7).
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.frap.select_elements(type='EDGE', indices=[0])
bpy.ops.mesh.extrude_edges_move(TRANSFORM_OT_translate={"value": (0, 0, -1)})
bpy.data.meshes['Cube'].total_vert_sel
bpy.data.meshes['Cube'].total_face_sel
