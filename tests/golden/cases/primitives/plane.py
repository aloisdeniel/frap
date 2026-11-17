# WP 1.19: mesh.primitive_plane_add at T1. Defaults at the 3D cursor; size, calc_uvs, location
# and rotation; align='VIEW' (the factory Layout viewport) with the ignored scale; a moved and
# rotated 3D cursor with align='CURSOR'; a set rotation winning over align='VIEW'.
# golden: startup=factory compare=objects,context,mesh:Plane,mesh:Plane.001,mesh:Plane.002,mesh:Plane.003,mesh:Plane.004 tier=T1
import bpy
bpy.ops.mesh.primitive_plane_add()
bpy.ops.mesh.primitive_plane_add(size=3.5, calc_uvs=False, location=(1, -2, 0.5), rotation=(0.3, 0.2, -1.1))
bpy.ops.mesh.primitive_plane_add(align='VIEW', scale=(2, 3, 4))
bpy.context.scene.cursor.location = (1, 2, 3)
bpy.context.scene.cursor.rotation_euler = (0.2, 0.3, 0.4)
bpy.ops.mesh.primitive_plane_add(align='CURSOR', size=0.25)
bpy.ops.mesh.primitive_plane_add(align='VIEW', rotation=(0.1, 0, 0))
