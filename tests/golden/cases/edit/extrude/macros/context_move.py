# PENDING (WP 2.5 -> WP 2.12): enable by renaming to `.py` once `transform.translate` and
# `transform.shrink_fatten` work in Edit Mode (TRANSFORM_MODE_HANDLERS entry of WP 2.12); the
# Blender fixture is already blessed (tests/golden/fixtures/edit/extrude/macros/).
# WP 2.5: mesh.extrude_context_move on two adjacent faces (V14 E24 F12).
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1 order=canonical
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[0, 4])
bpy.ops.mesh.extrude_context_move(TRANSFORM_OT_translate={"value": (0.5, 0, 0.5)})
bpy.data.meshes['Cube'].total_vert_sel
bpy.data.meshes['Cube'].total_face_sel
