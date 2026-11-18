# M2 W0 (M1 leftover): mathutils members on values read from RNA (§9.2): Matrix.translation,
# to_euler/to_quaternion/decompose/to_scale, median_scale, Vector length/normalized/angle/lerp
# and swizzles, Euler.to_quaternion, Quaternion.angle/axis. Each result is moved into an object
# transform so that it is compared.
# golden: startup=factory tier=T0 compare=objects
import bpy
bpy.data.objects['Cube'].location = bpy.data.objects['Camera'].matrix_world.translation
bpy.data.objects['Cube'].rotation_euler = bpy.data.objects['Camera'].matrix_world.to_euler('ZYX')
bpy.data.objects['Cube'].scale = bpy.data.objects['Camera'].location.normalized()
bpy.data.objects['Light'].rotation_mode = 'QUATERNION'
bpy.data.objects['Light'].rotation_quaternion = bpy.data.objects['Camera'].matrix_world.to_quaternion()
bpy.data.objects['Light'].location.x = bpy.data.objects['Camera'].location.length
bpy.data.objects['Light'].location.y = bpy.data.objects['Camera'].location.angle((0, 0, 1))
bpy.data.objects['Light'].location.z = bpy.data.objects['Camera'].matrix_world.median_scale
bpy.data.objects['Light'].scale = bpy.data.objects['Camera'].location.zyx
bpy.data.objects['Light'].delta_location = bpy.data.objects['Camera'].matrix_world.decompose()[0]
bpy.data.objects['Light'].delta_scale = bpy.data.objects['Camera'].matrix_world.to_scale()
bpy.data.objects['Camera'].delta_location = bpy.data.objects['Camera'].location.lerp((0, 0, 0), 0.25)
bpy.data.objects['Camera'].delta_rotation_euler.x = bpy.data.objects['Camera'].rotation_euler.to_quaternion().angle
bpy.data.objects['Camera'].delta_rotation_euler.y = bpy.data.objects['Camera'].matrix_world.to_euler().z
bpy.data.objects['Camera'].delta_scale = bpy.data.objects['Camera'].rotation_euler.to_quaternion().axis
bpy.data.objects['Cube'].delta_scale.x = bpy.data.objects['Cube'].location.xyzw
