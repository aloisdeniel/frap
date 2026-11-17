# WP 1.21 (T0), object.parent_clear CLEAR (jumps to its channels), CLEAR_KEEP_TRANSFORM (keeps
# its world matrix), CLEAR_INVERSE (keeps the parent, resets the inverse).
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.data.objects['Light'].select_set(True)
bpy.data.objects['Camera'].select_set(True)
bpy.data.objects['Cube'].location = (1, 2, 3)
bpy.ops.object.parent_set()
bpy.data.objects['Cube'].location = (5, 2, 3)
bpy.data.objects['Cube'].scale = (1, 2, 1)
bpy.data.objects['Cube'].select_set(False)
bpy.data.objects['Camera'].select_set(False)
bpy.ops.object.parent_clear(type='CLEAR_KEEP_TRANSFORM')
bpy.data.objects['Light'].select_set(False)
bpy.data.objects['Camera'].select_set(True)
bpy.ops.object.parent_clear(type='CLEAR_INVERSE')
bpy.data.objects['Camera'].matrix_parent_inverse
bpy.ops.object.parent_clear()
