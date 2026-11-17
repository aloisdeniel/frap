# M1 stabilization: `Object.dimensions` (get and set) and `Object.bound_box` are evaluated
# bounds, always fresh (D-04): after a scale, a rotation and a parent's scale; objects without
# geometry read zeros; writing dimensions sets the scale (a zero dimension gives a zero scale).
# golden: startup=factory tier=T0 compare=objects
import bpy
bpy.data.objects['Cube'].scale = (2, -1, 3)
bpy.data.objects['Cube'].rotation_euler = (0.3, 0.2, 0.1)
bpy.data.objects['Light'].location = bpy.data.objects['Cube'].dimensions
bpy.data.objects['Light'].rotation_euler = bpy.data.objects['Cube'].bound_box[6]
bpy.data.objects['Light'].scale = bpy.data.objects['Camera'].dimensions
bpy.data.objects['Camera'].location = bpy.data.objects['Camera'].bound_box[0]
bpy.data.objects['Cube'].parent = bpy.data.objects['Camera']
bpy.data.objects['Camera'].scale = (2, 2, 2)
bpy.data.objects['Camera'].rotation_euler = bpy.data.objects['Cube'].dimensions
bpy.data.objects['Cube'].dimensions = (4, 5, 0)
bpy.data.objects['Cube'].dimensions.x = 3
bpy.data.objects['Cube'].bound_box = 1
bpy.data.objects['Cube'].bound_box
bpy.data.objects['Cube'].bound_box[8]
