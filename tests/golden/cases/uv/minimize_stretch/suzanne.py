# WP 8.1: uv.minimize_stretch on Suzanne's default UV map: with the island boundaries pinned,
# each of the 10 iterations moves every interior UV in a random direction (Blender's RNG, seed
# 31415926, so the result is reproducible) by the binary-searched step that lowers the texture
# stretch of its faces. From the same UVs frap's result is bit-identical to Blender's (the
# tolerance 1e-6 is for the other mesh data).
# golden: startup=empty compare=objects,context,mesh:Suzanne tier=T2 tol=1e-6
import bpy
bpy.ops.mesh.primitive_monkey_add()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.minimize_stretch(iterations=10)
bpy.ops.object.mode_set(mode='OBJECT')
