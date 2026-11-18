# WP 2.7: elements freed by mesh.delete are reused last-in first-out by the next operator
# that creates geometry (primitives merged into the edit mesh), so indices match Blender.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[1, 6])
bpy.ops.mesh.delete(type='VERT')
bpy.ops.mesh.primitive_cube_add(size=0.5, location=(3, 0, 0))
bpy.ops.frap.select_elements(type='FACE', indices=[0])
bpy.ops.mesh.delete(type='FACE')
bpy.ops.mesh.primitive_plane_add(size=1, location=(0, 3, 0))
