# WP 8.3.7: `object.shape_key_copy` (the active block duplicated after itself with its data,
# settings and flags, made active; the other blocks' `relative_key` indices are not shifted, so a
# block relative to the block after the copy now follows the copy, as in Blender),
# `shape_key_lock` (LOCK/UNLOCK, also in Edit Mode) and `shape_key_mirror` (spatial X mirror of the
# active block: pairs swap, centre vertices flip; a locked active block is an error), with their
# polls. The mix is compared on the cube (`evaluated:Cube`); single blocks through linked
# duplicates with `show_only_shape_key`; flags and indices through object transforms.
# golden: startup=factory tier=T1 compare=objects,evaluated:Cube,evaluated:Cube.001,evaluated:Cube.002,evaluated:Cube.003
import bpy
bpy.ops.object.shape_key_copy()
bpy.ops.object.shape_key_lock()
bpy.ops.object.shape_key_mirror()
bpy.ops.object.shape_key_add(from_mix=False)
bpy.ops.object.shape_key_add(from_mix=False)
bpy.ops.object.shape_key_add(from_mix=False)
bpy.ops.object.shape_key_add(from_mix=False)
bpy.ops.object.shape_key_add(from_mix=False)
bpy.data.shape_keys['Key'].key_blocks['Key 1'].data.foreach_set('co', [1.5, 1, 1, 1, 1.25, -1, 1, -1, 1, 1, -1, -1, -1, 1, 1.5, -1, 1, -1, -1, -1, 1, -1, -1, -1])
bpy.data.shape_keys['Key'].key_blocks['Key 2'].data.foreach_set('co', [1, 1, 2, 1, 1, -1, 1, -1, 1, 1, -1, -1, -1, 1, 1, -1, 1.5, -1, -1, -1, 1, -1, -1, -1])
bpy.data.shape_keys['Key'].key_blocks['Key 3'].data.foreach_set('co', [1, 1, 1, 1, 1, -1, 1, -1, 1, 1, -1, -1, -2, 1, 1, -1, 1, -1, -1, -1, 1, -1, -1, -1])
bpy.data.shape_keys['Key'].key_blocks['Key 4'].data.foreach_set('co', [1, 1, 1, 1, 1, -1, 1, -1, 1, 1, -1, -1, -2, 1, 1, -1, 1, -1, -1, -1.5, 1, -1, -1, -1])
bpy.data.shape_keys['Key'].key_blocks['Key 3'].relative_key = bpy.data.shape_keys['Key'].key_blocks['Key 1']
bpy.data.shape_keys['Key'].key_blocks['Key 4'].relative_key = bpy.data.shape_keys['Key'].key_blocks['Key 3']
bpy.data.shape_keys['Key'].key_blocks['Key 1'].value = 0.5
bpy.data.shape_keys['Key'].key_blocks['Key 2'].value = 0.25
bpy.data.shape_keys['Key'].key_blocks['Key 2'].slider_max = 2.0
bpy.data.shape_keys['Key'].key_blocks['Key 2'].interpolation = 'KEY_CARDINAL'
bpy.data.shape_keys['Key'].key_blocks['Key 2'].mute = True
bpy.data.shape_keys['Key'].key_blocks['Key 3'].value = 0.0
bpy.data.shape_keys['Key'].key_blocks['Key 4'].value = 1.0
bpy.data.objects['Cube'].active_shape_key_index = 2
bpy.ops.object.shape_key_copy()
bpy.data.objects['Camera'].location = (bpy.data.objects['Cube'].active_shape_key_index, bpy.data.shape_keys['Key'].key_blocks['Key 2.001'].value, bpy.data.shape_keys['Key'].key_blocks['Key 2.001'].slider_max)
bpy.data.objects['Camera'].scale = (bpy.data.shape_keys['Key'].key_blocks['Key 2.001'].mute, bpy.data.shape_keys['Key'].key_blocks['Key 4'].value, bpy.data.shape_keys['Key'].key_blocks[3].value)
bpy.data.shape_keys['Key'].key_blocks['Key 2.001'].interpolation
bpy.data.shape_keys['Key'].key_blocks['Key 2.001'].mute = False
bpy.data.shape_keys['Key'].key_blocks['Key 2.001'].value = 0.5
bpy.ops.object.shape_key_lock(action='LOCK')
bpy.data.objects['Cube'].active_shape_key_index = 1
bpy.ops.object.shape_key_mirror()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.object.shape_key_copy()
bpy.ops.object.shape_key_mirror()
bpy.ops.object.shape_key_lock(action='UNLOCK')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Camera'].rotation_euler = (bpy.data.shape_keys['Key'].key_blocks['Key 1'].lock_shape, bpy.data.shape_keys['Key'].key_blocks['Basis'].lock_shape, 0)
bpy.ops.object.shape_key_mirror()
bpy.ops.object.shape_key_lock()
bpy.data.objects['Light'].scale = (bpy.data.shape_keys['Key'].key_blocks['Key 4'].lock_shape, 1, 1)
bpy.ops.object.shape_key_lock(action='UNLOCK')
bpy.data.objects['Cube'].active_shape_key_index = 0
bpy.ops.object.shape_key_mirror()
bpy.ops.object.duplicate(linked=True)
bpy.data.objects['Cube.001'].show_only_shape_key = True
bpy.data.objects['Cube.001'].active_shape_key_index = 1
bpy.ops.object.duplicate(linked=True)
bpy.data.objects['Cube.002'].show_only_shape_key = True
bpy.data.objects['Cube.002'].active_shape_key_index = 3
bpy.ops.object.duplicate(linked=True)
bpy.data.objects['Cube.003'].show_only_shape_key = True
bpy.data.objects['Cube.003'].active_shape_key_index = 0
