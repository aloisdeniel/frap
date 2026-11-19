# WP 3.8: uv.reset gives every selected face the default layout (quads (0,0) (1,0) (1,1)
# (0,1) from their first corner, n-gons on a circle); unselected faces keep their UVs.
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Circle tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.smart_project()
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[0, 4])
bpy.ops.uv.reset()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_circle_add(vertices=7, fill_type='NGON', location=(3, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.reset()
bpy.ops.object.mode_set(mode='OBJECT')
