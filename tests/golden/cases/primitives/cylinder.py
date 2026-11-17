# WP 1.19: mesh.primitive_cylinder_add at T1. Defaults; vertices/radius/depth with NOTHING and
# TRIFAN fills; location, rotation and a non-uniform scale; align='VIEW'; align='CURSOR' with a
# moved and rotated 3D cursor.
# golden: startup=factory compare=objects,context,mesh:Cylinder,mesh:Cylinder.001,mesh:Cylinder.002,mesh:Cylinder.003,mesh:Cylinder.004 tier=T1
import bpy
bpy.ops.mesh.primitive_cylinder_add()
bpy.ops.mesh.primitive_cylinder_add(vertices=5, radius=2, depth=0.5, end_fill_type='NOTHING', location=(0, 0, 2))
bpy.ops.mesh.primitive_cylinder_add(vertices=6, depth=3, end_fill_type='TRIFAN', calc_uvs=False, rotation=(1.5708, 0, 0), scale=(1, 2, 0.5))
bpy.ops.mesh.primitive_cylinder_add(vertices=8, align='VIEW')
bpy.context.scene.cursor.location = (0.5, -1, 0)
bpy.context.scene.cursor.rotation_euler = (0, 0.8, 0)
bpy.ops.mesh.primitive_cylinder_add(vertices=16, radius=0.5, depth=2, location=(0, 0, 1), align='CURSOR')
