# WP 1.19: mesh.primitive_grid_add at T1. Defaults (10 × 10); subdivisions and size with a
# location; a 1 × 1 grid without UVs, rotated, with the ignored scale; align='VIEW';
# align='CURSOR' with a moved and rotated 3D cursor.
# golden: startup=factory compare=objects,context,mesh:Grid,mesh:Grid.001,mesh:Grid.002,mesh:Grid.003,mesh:Grid.004 tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add()
bpy.ops.mesh.primitive_grid_add(x_subdivisions=3, y_subdivisions=5, size=4, location=(0, 0, -1))
bpy.ops.mesh.primitive_grid_add(x_subdivisions=1, y_subdivisions=1, calc_uvs=False, rotation=(0.2, 0.4, 0.6), scale=(5, 5, 5))
bpy.ops.mesh.primitive_grid_add(x_subdivisions=7, y_subdivisions=2, size=0.5, align='VIEW')
bpy.context.scene.cursor.location = (4, 4, 0)
bpy.context.scene.cursor.rotation_euler = (0, 0, 0.785)
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=4, align='CURSOR')
