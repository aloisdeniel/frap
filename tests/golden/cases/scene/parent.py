# WP 1.10: `Object.parent` assignment resets `matrix_parent_inverse` to identity (the child
# jumps); a cycle clears the parent silently; the object itself is a TypeError. Matrix tuples are
# columns (memory order).
# golden: startup=factory tier=T0 compare=objects tol=1e-5
import bpy
bpy.data.objects['Cube'].matrix_parent_inverse = ((1, 0, 0, 0), (0, 1, 0, 0), (0, 0, 1, 0), (1, 2, 3, 1))
bpy.data.objects['Cube'].parent = bpy.data.objects['Light']
bpy.data.objects['Light'].parent = bpy.data.objects['Cube']
bpy.data.objects['Cube'].parent = bpy.data.objects['Cube']
bpy.data.objects['Camera'].parent = bpy.data.objects['Cube']
