# WP 1.26: writes of parts of math-typed properties: named components (`x`, `w`), indices
# (also negative), slices of vectors, Eulers and quaternions, and a list given for a vector.
# golden: startup=factory tier=T0 compare=objects
import bpy
bpy.data.objects['Cube'].location.x = 2
bpy.data.objects['Cube'].location[1] = 3.5
bpy.data.objects['Cube'].location[-1] = -1
bpy.data.objects['Cube'].scale[0:2] = (5, 6)
bpy.data.objects['Cube'].scale[1:] = [7, 8]
bpy.data.objects['Cube'].rotation_euler.z = 1
bpy.data.objects['Cube'].rotation_euler[0] = 0.5
bpy.data.objects['Light'].rotation_mode = 'QUATERNION'
bpy.data.objects['Light'].rotation_quaternion.w = 0.5
bpy.data.objects['Light'].rotation_quaternion[1:3] = (0.5, 0.5)
bpy.data.objects['Light'].rotation_quaternion.z = 0.5
bpy.data.objects['Camera'].location = [1, 2.5, 3]
bpy.data.objects['Camera'].scale = (1, 2, 3)
bpy.data.objects['Camera'].scale.y = bpy.data.objects['Cube'].location.y
