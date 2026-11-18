# WP 2.13 workflow (shelf unit): a board whose top face is extruded four times in one call
# with mesh.extrude_repeat to make a stack of five boxes, then the five front faces (picked
# by their normal) inset individually as the shelf openings, pushed in with a negative depth.
# golden: startup=empty compare=objects,context,mesh:Cube tier=T1 order=canonical
import bpy
bpy.ops.mesh.primitive_cube_add(size=1, location=(0, 0, 0.1), scale=(1, 0.4, 0.2))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_by_normal(direction=(0, 0, 1))
bpy.ops.mesh.extrude_repeat(steps=4, offset=(0, 0, 0.3))
bpy.data.meshes['Cube'].total_face_sel
bpy.ops.frap.select_by_normal(direction=(0, -1, 0))
bpy.data.meshes['Cube'].total_face_sel
bpy.ops.mesh.inset(thickness=0.03, depth=-0.15, use_individual=True)
bpy.data.meshes['Cube'].total_face_sel
