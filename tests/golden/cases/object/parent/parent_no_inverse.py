# WP 1.21 (T0), object.parent_no_inverse_set: identity parent inverse, location reset (the
# child sits at the parent's origin); keep_transform=True keeps the world matrix with
# parent inverse = parent^-1 * orthonormal(parent).
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.data.objects['Cube'].location = (1, 2, 3)
bpy.data.objects['Cube'].rotation_euler = (0, 0, 0.5)
bpy.data.objects['Cube'].scale = (2, 1, 1)
bpy.data.objects['Light'].select_set(True)
bpy.ops.object.parent_no_inverse_set()
bpy.data.objects['Light'].select_set(False)
bpy.data.objects['Camera'].select_set(True)
bpy.ops.object.parent_no_inverse_set(keep_transform=True)
bpy.data.objects['Camera'].matrix_parent_inverse
