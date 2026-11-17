# WP 1.21 (T0), object.parent_set: the active object parents the other selected ones with the
# inverse of its world matrix; nothing moves; selection and active object stay.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.data.objects['Cube'].location = (1, 2, 3)
bpy.data.objects['Cube'].rotation_euler = (0, 0, 0.5)
bpy.data.objects['Light'].select_set(True)
bpy.data.objects['Camera'].select_set(True)
bpy.ops.object.parent_set()
bpy.data.objects['Light'].matrix_parent_inverse
bpy.data.objects['Light'].parent_type
