# PENDING (WP 2.5 -> WP 2.12): enable by renaming to `.py` once `transform.translate` and
# `transform.shrink_fatten` work in Edit Mode (TRANSFORM_MODE_HANDLERS entry of WP 2.12); the
# Blender fixture is already blessed (tests/golden/fixtures/edit/extrude/macros/).
# WP 2.5: mesh.extrude_region_shrink_fatten on the top face (z = 1.5 for 0.5).
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1 order=canonical
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[0])
bpy.ops.mesh.extrude_region_shrink_fatten(TRANSFORM_OT_shrink_fatten={"value": 0.5})
bpy.data.meshes['Cube'].total_vert_sel
bpy.data.meshes['Cube'].total_face_sel
