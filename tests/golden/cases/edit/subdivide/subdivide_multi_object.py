# WP 2.6: mesh.subdivide in multi-object Edit Mode: each edit mesh with a selected edge or face
# is subdivided; meshes with nothing selected are left alone.
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Plane,mesh:Cylinder tier=T1
import bpy
bpy.ops.mesh.primitive_plane_add(location=(3, 0, 0))
bpy.ops.mesh.primitive_cylinder_add(vertices=6, location=(-3, 0, 0))
bpy.ops.object.select_all(action='SELECT')
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.subdivide(number_cuts=1)
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[0])
bpy.ops.mesh.subdivide(number_cuts=2)
