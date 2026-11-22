# WP 8.3.7: `object.shape_key_make_basis` (the active block moves to the top at 100%, the old and
# the new basis become relative to the new basis, the other blocks' relative keys and the active
# index follow, the mesh takes the new basis) and `object.shape_key_apply_to_basis` (the selected
# and the active blocks, locked ones skipped, mixed into the basis at their values and vertex
# groups and removed; the blocks depending on the basis keep their shapes on top of it; the
# basis becomes active), with their polls. Every block has its own `slider_max`, read back by
# index to show the block order; the mix and the mesh are compared (`evaluated:Cube`,
# `mesh:Cube`), single blocks through linked duplicates with `show_only_shape_key`.
# golden: startup=factory tier=T1 compare=objects,mesh:Cube,evaluated:Cube,evaluated:Cube.001,evaluated:Cube.002
import bpy
bpy.ops.object.shape_key_make_basis()
bpy.ops.object.shape_key_apply_to_basis()
bpy.data.objects['Cube'].vertex_groups.new(name='W')
bpy.data.objects['Cube'].vertex_groups['W'].add([0, 1, 2, 3], 0.5, 'REPLACE')
bpy.data.objects['Cube'].vertex_groups['W'].add([4], 1.0, 'REPLACE')
bpy.ops.object.shape_key_add(from_mix=False)
bpy.ops.object.shape_key_add(from_mix=False)
bpy.ops.object.shape_key_add(from_mix=False)
bpy.ops.object.shape_key_add(from_mix=False)
bpy.ops.object.shape_key_add(from_mix=False)
bpy.data.shape_keys['Key'].key_blocks['Key 1'].data.foreach_set('co', [1.5, 1, 1, 1, 1, -1, 1, -1, 1.5, 1, -1, -1, -1, 1, 1, -1, 1, -1, -1, -1, 1, -1, -1, -1.25])
bpy.data.shape_keys['Key'].key_blocks['Key 2'].data.foreach_set('co', [1.5, 1.5, 1, 1, 1, -1, 1, -1, 1.5, 1, -1, -1, -1, 1, 1, -1, 1, -1.5, -1, -1, 1, -1, -1, -1.25])
bpy.data.shape_keys['Key'].key_blocks['Key 3'].data.foreach_set('co', [1, 1, 1.75, 1, 1, -1, 1, -1, 1, 1, -1, -1, -1.5, 1, 1, -1, 1, -1, -1, -1, 1, -1, -1, -1])
bpy.data.shape_keys['Key'].key_blocks['Key 4'].data.foreach_set('co', [1, 1, 1, 1.5, 1, -1, 1, -1, 1, 1, -1, -1, -1, 1, 1, -1, 1, -1, -1, -1.5, 1, -1, -1, -1])
bpy.data.shape_keys['Key'].key_blocks['Key 2'].relative_key = bpy.data.shape_keys['Key'].key_blocks['Key 1']
bpy.data.shape_keys['Key'].key_blocks['Key 3'].vertex_group = 'W'
bpy.data.shape_keys['Key'].key_blocks['Basis'].slider_max = 2.0
bpy.data.shape_keys['Key'].key_blocks['Key 1'].slider_max = 3.0
bpy.data.shape_keys['Key'].key_blocks['Key 2'].slider_max = 4.0
bpy.data.shape_keys['Key'].key_blocks['Key 3'].slider_max = 5.0
bpy.data.shape_keys['Key'].key_blocks['Key 4'].slider_max = 6.0
bpy.data.shape_keys['Key'].key_blocks['Key 1'].value = 0.3
bpy.data.shape_keys['Key'].key_blocks['Key 2'].value = 0.5
bpy.data.shape_keys['Key'].key_blocks['Key 3'].value = 0.8
bpy.data.shape_keys['Key'].key_blocks['Key 4'].value = 1.0
bpy.data.objects['Cube'].active_shape_key_index = 0
bpy.ops.object.shape_key_make_basis()
bpy.data.objects['Cube'].active_shape_key_index = 1
bpy.ops.object.shape_key_make_basis()
bpy.data.objects['Camera'].location = (bpy.data.shape_keys['Key'].key_blocks[0].slider_max, bpy.data.shape_keys['Key'].key_blocks[1].slider_max, bpy.data.objects['Cube'].active_shape_key_index)
bpy.data.objects['Camera'].scale = (bpy.data.shape_keys['Key'].key_blocks[0].relative_key.slider_max, bpy.data.shape_keys['Key'].key_blocks[1].relative_key.slider_max, bpy.data.shape_keys['Key'].key_blocks[2].relative_key.slider_max)
bpy.data.shape_keys['Key'].reference_key.name
bpy.data.objects['Cube'].active_shape_key_index = 3
bpy.ops.object.shape_key_make_basis()
bpy.data.objects['Camera'].rotation_euler = (bpy.data.shape_keys['Key'].key_blocks[0].slider_max, bpy.data.shape_keys['Key'].key_blocks[3].slider_max, bpy.data.shape_keys['Key'].key_blocks[2].relative_key.slider_max)
bpy.ops.object.duplicate(linked=True)
bpy.data.objects['Cube.001'].show_only_shape_key = True
bpy.data.objects['Cube.001'].active_shape_key_index = 2
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Cube'].select_set(True)
bpy.data.objects['Cube'].active_shape_key_index = 4
bpy.data.shape_keys['Key'].key_blocks[1].select = True
bpy.data.shape_keys['Key'].key_blocks[2].select = True
bpy.data.shape_keys['Key'].key_blocks[2].lock_shape = True
bpy.ops.object.shape_key_apply_to_basis()
bpy.data.objects['Light'].location = (bpy.data.shape_keys['Key'].key_blocks[0].slider_max, bpy.data.shape_keys['Key'].key_blocks[1].slider_max, bpy.data.objects['Cube'].active_shape_key_index)
bpy.data.objects['Light'].scale = (bpy.data.shape_keys['Key'].key_blocks[2].slider_max, bpy.data.shape_keys['Key'].key_blocks[1].relative_key.slider_max, bpy.data.shape_keys['Key'].key_blocks[2].relative_key.slider_max)
bpy.data.shape_keys['Key'].key_blocks[3]
bpy.ops.object.shape_key_apply_to_basis()
bpy.data.objects['Cube'].active_shape_key_index = 1
bpy.data.shape_keys['Key'].key_blocks[1].lock_shape = True
bpy.data.shape_keys['Key'].key_blocks[2].lock_shape = False
bpy.data.shape_keys['Key'].key_blocks[2].select = False
bpy.ops.object.shape_key_apply_to_basis()
bpy.data.shape_keys['Key'].key_blocks[1].lock_shape = False
bpy.data.shape_keys['Key'].key_blocks[1].mute = True
bpy.ops.object.shape_key_apply_to_basis()
bpy.ops.object.duplicate(linked=True)
bpy.data.objects['Cube.002'].show_only_shape_key = True
bpy.data.objects['Cube.002'].active_shape_key_index = 1
