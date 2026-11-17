# WP 1.26: vector operator arguments of the wrong length (code OP_ARG_LENGTH, ValueError).
# golden: startup=factory tier=T0 compare=objects
import bpy
bpy.ops.mesh.primitive_cube_add(location=(1, 2))
bpy.ops.mesh.primitive_cube_add(location=(1, 2, 3, 4))
bpy.ops.mesh.primitive_cube_add(rotation=(0, 0))
bpy.ops.mesh.primitive_cube_add(scale=[1, 1, 1, 1, 1])
bpy.ops.mesh.primitive_plane_add(location=())
bpy.ops.mesh.primitive_plane_add(location=[2, 0, 0])
