# WP 1.19: mesh.primitive_cone_add at T1. Defaults (pointed); a truncated cone with NOTHING fill;
# a zero bottom radius with TRIFAN fill, rotation and scale; align='VIEW'; align='CURSOR' with a
# moved and rotated 3D cursor.
# golden: startup=factory compare=objects,context,mesh:Cone,mesh:Cone.001,mesh:Cone.002,mesh:Cone.003,mesh:Cone.004 tier=T1
import bpy
bpy.ops.mesh.primitive_cone_add()
bpy.ops.mesh.primitive_cone_add(vertices=8, radius1=1, radius2=0.5, depth=1, end_fill_type='NOTHING', location=(2, 0, 0))
bpy.ops.mesh.primitive_cone_add(vertices=6, radius1=0, radius2=1, end_fill_type='TRIFAN', calc_uvs=False, rotation=(0, 0.4, 0.2), scale=(1, 1, 2))
bpy.ops.mesh.primitive_cone_add(vertices=5, align='VIEW')
bpy.context.scene.cursor.location = (0, 0, -1)
bpy.context.scene.cursor.rotation_euler = (3.0, 0, 0)
bpy.ops.mesh.primitive_cone_add(vertices=12, radius1=2, depth=4, align='CURSOR')
