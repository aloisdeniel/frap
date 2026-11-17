# WP 1.19: mesh.primitive_ico_sphere_add at T1. Defaults; subdivisions and radius with a
# location; rotation with a non-uniform scale and no UVs; align='VIEW'; align='CURSOR' with a
# moved and rotated 3D cursor.
# golden: startup=factory compare=objects,context,mesh:Icosphere,mesh:Icosphere.001,mesh:Icosphere.002,mesh:Icosphere.003,mesh:Icosphere.004 tier=T1
import bpy
bpy.ops.mesh.primitive_ico_sphere_add()
bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=1, radius=2, location=(0, 3, 0))
bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=3, radius=0.5, calc_uvs=False, rotation=(0.7, 0, -0.3), scale=(2, 1, 1))
bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=1, align='VIEW')
bpy.context.scene.cursor.location = (-2, 0, 1)
bpy.context.scene.cursor.rotation_euler = (0.5, 0.5, 0.5)
bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=2, align='CURSOR')
