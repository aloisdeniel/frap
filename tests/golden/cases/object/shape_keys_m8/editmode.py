# WP 8.3.7: `object.shape_key_make_basis` and `shape_key_apply_to_basis` called in Edit Mode (their
# polls allow it): they change the key and the mesh data, and leaving Edit Mode then writes the
# edit mesh back over them (the block that was active on entering takes the edited positions,
# by index, as in Blender). `shape_key_lock` works in Edit Mode too. Blocks are compared through
# linked duplicates with `show_only_shape_key`, the mesh as `mesh:Cube`.
# golden: startup=factory tier=T1 compare=objects,mesh:Cube,evaluated:Cube.001,evaluated:Cube.002,evaluated:Cube.003
import bpy
bpy.ops.object.shape_key_add(from_mix=False)
bpy.ops.object.shape_key_add(from_mix=False)
bpy.ops.object.shape_key_add(from_mix=False)
bpy.data.shape_keys['Key'].key_blocks['Key 1'].data[0].co = (2, 2, 2)
bpy.data.shape_keys['Key'].key_blocks['Key 2'].data[1].co = (1.5, 1.5, -1.5)
bpy.data.shape_keys['Key'].key_blocks['Key 2'].value = 0.5
bpy.data.objects['Cube'].active_shape_key_index = 1
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.object.shape_key_make_basis()
bpy.ops.object.shape_key_lock()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Camera'].location = (bpy.data.objects['Cube'].active_shape_key_index, bpy.data.shape_keys['Key'].key_blocks[0].lock_shape, 0)
bpy.ops.object.shape_key_lock(action='UNLOCK')
bpy.data.objects['Cube'].active_shape_key_index = 2
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.object.shape_key_apply_to_basis()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Camera'].scale = (bpy.data.objects['Cube'].active_shape_key_index, 1, 1)
bpy.ops.object.duplicate(linked=True)
bpy.data.objects['Cube.001'].show_only_shape_key = True
bpy.data.objects['Cube.001'].active_shape_key_index = 0
bpy.ops.object.duplicate(linked=True)
bpy.data.objects['Cube.002'].show_only_shape_key = True
bpy.data.objects['Cube.002'].active_shape_key_index = 1
bpy.ops.object.duplicate(linked=True)
bpy.data.objects['Cube.003'].show_only_shape_key = True
bpy.data.objects['Cube.003'].active_shape_key_index = 2
