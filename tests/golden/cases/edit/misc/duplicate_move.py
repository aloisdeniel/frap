# WP 2.10: mesh.duplicate_move (needs the edit-mode translate of WP 2.12): the copy of the top
# face is appended, selected and moved up by 1; the originals stay and are deselected; the
# short form flattens the macro arguments.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[5])
bpy.ops.mesh.duplicate_move(MESH_OT_duplicate={'mode': 1}, TRANSFORM_OT_translate={'value': (0, 0, 1)})
bpy.data.meshes['Cube'].total_vert_sel
bpy.ops.frap.select_elements(type='EDGE', indices=[0, 1])
bpy.ops.mesh.duplicate_move(TRANSFORM_OT_translate={'value': (0.5, -0.25, 0)})
