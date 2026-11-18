# WP 2.11: vertex bevel of plane corners (two edges each: no vertex mesh, a curved
# edge with three segments) and of a circle vertex (no face: new edges).
# golden: startup=factory compare=objects,context,mesh:Plane,mesh:Circle tier=T2 order=exact
import bpy
bpy.ops.mesh.primitive_plane_add()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 3])
bpy.ops.mesh.bevel(offset=0.3, segments=3, affect='VERTICES')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_circle_add(vertices=6, location=(3, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[1])
bpy.ops.mesh.bevel(offset=0.2, segments=2, affect='VERTICES')
