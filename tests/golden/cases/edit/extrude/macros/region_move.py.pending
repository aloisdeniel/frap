# PENDING (WP 2.5 -> WP 2.12): enable by renaming to `.py` once `transform.translate` and
# `transform.shrink_fatten` work in Edit Mode (TRANSFORM_MODE_HANDLERS entry of WP 2.12); the
# Blender fixture is already blessed (tests/golden/fixtures/edit/extrude/macros/).
# WP 2.5: mesh.extrude_region_move (macro: extrude_region + translate) on the top face, then
# again with nested arguments for both steps (bpy dict spelling; the transpile test checks the
# short form `MESH_OT_extrude_region.use_normal_flip=True TRANSFORM_OT_translate.value=…`).
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1 order=canonical
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[0])
bpy.ops.mesh.extrude_region_move(TRANSFORM_OT_translate={"value": (0, 0, 1)})
bpy.ops.mesh.extrude_region_move(MESH_OT_extrude_region={"use_normal_flip": True}, TRANSFORM_OT_translate={"value": (0.5, 0, 0)})
bpy.data.meshes['Cube'].total_vert_sel
bpy.data.meshes['Cube'].total_face_sel
