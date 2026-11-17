# WP 1.26: sequences of the wrong length (code RNA_LENGTH, ValueError) for vectors, Eulers,
# quaternions, colours, socket colours, matrices (dimension 1 for a short row) and vector slice
# assignment.
# golden: startup=factory tier=T0 compare=objects,materials
import bpy
bpy.data.objects['Cube'].location = (1, 2)
bpy.data.objects['Cube'].scale = (1, 2, 3, 4)
bpy.data.objects['Cube'].rotation_euler = (1, 2)
bpy.data.objects['Cube'].rotation_quaternion = (1, 0, 0)
bpy.data.materials['Material'].diffuse_color = (1, 1, 1)
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (1, 0, 0)
bpy.data.objects['Cube'].matrix_world = ((1, 0, 0), (0, 1, 0), (0, 0, 1))
bpy.data.objects['Cube'].location[0:2] = (1, 2, 3)
bpy.data.scenes['Scene'].cursor.location = (1, 2)
