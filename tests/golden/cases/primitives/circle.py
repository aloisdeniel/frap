# WP 1.19: mesh.primitive_circle_add at T1. Defaults (open ring); NGON and TRIFAN fills with
# vertices and radius; location/rotation; align='VIEW' with the ignored scale; align='CURSOR'
# with a 3D cursor in quaternion mode.
# golden: startup=factory compare=objects,context,mesh:Circle,mesh:Circle.001,mesh:Circle.002,mesh:Circle.003,mesh:Circle.004 tier=T1
import bpy
bpy.ops.mesh.primitive_circle_add()
bpy.ops.mesh.primitive_circle_add(vertices=5, radius=2, fill_type='NGON', location=(1, 1, 0))
bpy.ops.mesh.primitive_circle_add(vertices=7, radius=0.5, fill_type='TRIFAN', calc_uvs=False, rotation=(1.0, 0.0, 0.5))
bpy.ops.mesh.primitive_circle_add(vertices=12, align='VIEW', scale=(3, 3, 3))
bpy.context.scene.cursor.rotation_mode = 'QUATERNION'
bpy.context.scene.cursor.rotation_quaternion = (0.9, 0.1, 0.3, -0.2)
bpy.context.scene.cursor.location = (0, -3, 1)
bpy.ops.mesh.primitive_circle_add(vertices=6, fill_type='NGON', align='CURSOR')
