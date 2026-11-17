# WP 1.19: mesh.primitive_cube_add at T1. Defaults; size and calc_uvs with a location; a
# rotation with a non-uniform scale (baked into the mesh, object scale stays 1); align='VIEW';
# an explicit zero scale component; align='CURSOR' with a moved and rotated 3D cursor.
# golden: startup=factory compare=objects,context,mesh:Cube.001,mesh:Cube.002,mesh:Cube.003,mesh:Cube.004,mesh:Cube.005,mesh:Cube.006 tier=T1
import bpy
bpy.ops.mesh.primitive_cube_add()
bpy.ops.mesh.primitive_cube_add(size=3, calc_uvs=False, location=(0, 0, 1))
bpy.ops.mesh.primitive_cube_add(size=1, location=(2, 0, 0), rotation=(0.5, -0.25, 2.0), scale=(1, 2, 3))
bpy.ops.mesh.primitive_cube_add(align='VIEW')
bpy.ops.mesh.primitive_cube_add(scale=(2, 0, 1))
bpy.context.scene.cursor.location = (-1, 0.5, 2)
bpy.context.scene.cursor.rotation_euler = (0.2, 0.3, 0.4)
bpy.ops.mesh.primitive_cube_add(align='CURSOR', size=0.5)
