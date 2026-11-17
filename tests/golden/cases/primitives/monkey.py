# WP 1.19: mesh.primitive_monkey_add (Suzanne) at T1 (licence decision D1). Defaults; size with
# a location; no UVs, rotated, with the ignored scale; align='VIEW'; align='CURSOR' with a moved
# and rotated 3D cursor.
# golden: startup=factory compare=objects,context,mesh:Suzanne,mesh:Suzanne.001,mesh:Suzanne.002,mesh:Suzanne.003,mesh:Suzanne.004 tier=T1
import bpy
bpy.ops.mesh.primitive_monkey_add()
bpy.ops.mesh.primitive_monkey_add(size=0.5, location=(0, 2, 0))
bpy.ops.mesh.primitive_monkey_add(size=4, calc_uvs=False, rotation=(0, 0, 3.14159), scale=(2, 2, 2))
bpy.ops.mesh.primitive_monkey_add(align='VIEW')
bpy.context.scene.cursor.location = (1, 1, 1)
bpy.context.scene.cursor.rotation_euler = (0.1, 0.2, 0.3)
bpy.ops.mesh.primitive_monkey_add(align='CURSOR')
