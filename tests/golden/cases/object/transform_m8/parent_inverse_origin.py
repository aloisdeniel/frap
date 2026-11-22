# WP 8.3.5 (T1), object.parent_inverse_apply (the parent's orthonormal transform becomes the
# child's origin, the world matrix stays; a parent and its child selected together use the
# matrices from before the call), object.origin_clear (a parented child moves onto its parent's
# origin; unparented objects stay) and their polls.
# golden: startup=factory compare=objects,context tier=T1 tol=1e-5
import bpy
bpy.data.objects['Cube'].location = (1, 2, 3)
bpy.data.objects['Cube'].rotation_euler = (0.2, 0.3, 0.5)
bpy.data.objects['Cube'].scale = (2, 1, 0.5)
bpy.data.objects['Light'].select_set(True)
bpy.ops.object.parent_set(type='OBJECT', keep_transform=True)
bpy.data.objects['Camera'].parent = bpy.data.objects['Light']
bpy.data.objects['Camera'].select_set(True)
bpy.ops.object.parent_inverse_apply()
bpy.data.objects['Light'].location = (0.5, 0.5, 0.5)
bpy.ops.object.origin_clear()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.object.parent_inverse_apply()
bpy.ops.object.origin_clear()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.object.select_all(action='DESELECT')
bpy.ops.object.parent_inverse_apply()
bpy.ops.object.origin_clear()
