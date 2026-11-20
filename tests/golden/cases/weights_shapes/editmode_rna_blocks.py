# WP 6.3: `Object.shape_key_add` / `shape_key_remove` called in Edit Mode. Layers are matched
# to blocks by uid (`KeyBlock.uid`), the edited block is the one the edit mesh was made with
# (`bm->shapenr`), a block added while editing is rebuilt through the original vertex indices,
# and a block removed while editing is re-added from its layer (`bm_to_mesh_shape`). Then
# `ob.evaluated_get(dg).data` of an object with a key and no modifier reads the mix, and
# `active_shape_key_index = -1` leaves no active block while the index reads 0.
# golden: startup=factory tier=T1 compare=objects,mesh:Cube,evaluated:Cube.001,evaluated:Cube.002,evaluated:Cube.003,evaluated:Cube.004
import bpy
bpy.ops.object.duplicate(linked=True)
bpy.ops.object.duplicate(linked=True)
bpy.ops.object.duplicate(linked=True)
bpy.ops.object.duplicate(linked=True)
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Cube'].select_set(True)
bpy.data.objects['Cube'].shape_key_add(name='Basis')
bpy.data.objects['Cube'].shape_key_add(name='K1', from_mix=False)
bpy.data.objects['Cube'].shape_key_add(name='K2', from_mix=False)
bpy.data.shape_keys['Key'].key_blocks['K1'].data.foreach_set('co', [2, 2, 2, 2, 2, 0, 2, 0, 2, 2, 0, 0, 0, 2, 2, 0, 2, 0, 0, 0, 2, 0, 0, 0])
bpy.data.shape_keys['Key'].key_blocks['K2'].data.foreach_set('co', [2, 3, 2, 2, 3, 0, 2, 1, 2, 2, 1, 0, 0, 3, 2, 0, 3, 0, 0, 1, 2, 0, 1, 0])
bpy.data.shape_keys['Key'].key_blocks['K2'].relative_key = bpy.data.shape_keys['Key'].key_blocks['K1']
bpy.data.objects['Cube'].active_shape_key_index = 1
bpy.ops.object.mode_set(mode='EDIT')
bpy.data.objects['Cube'].shape_key_add(name='E1')
bpy.data.objects['Cube'].shape_key_remove(bpy.data.shape_keys['Key'].key_blocks['K2'])
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.transform.translate(value=(0, 0, 1))
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Camera'].scale = (bpy.data.shape_keys['Key'].key_blocks[3].value + 4, bpy.data.objects['Cube'].active_shape_key_index, 1)
bpy.data.shape_keys['Key'].key_blocks[4]
bpy.data.shape_keys['Key'].key_blocks[3].name
bpy.data.objects['Cube.001'].show_only_shape_key = True
bpy.data.objects['Cube.001'].active_shape_key_index = 0
bpy.data.objects['Cube.002'].show_only_shape_key = True
bpy.data.objects['Cube.002'].active_shape_key_index = 1
bpy.data.objects['Cube.003'].show_only_shape_key = True
bpy.data.objects['Cube.003'].active_shape_key_index = 2
bpy.data.objects['Cube.004'].show_only_shape_key = True
bpy.data.objects['Cube.004'].active_shape_key_index = 3
bpy.data.shape_keys['Key'].key_blocks['K1'].value = 1.0
bpy.data.objects['Light'].location = bpy.data.objects['Cube'].evaluated_get(bpy.context.evaluated_depsgraph_get()).data.vertices[0].co
bpy.data.objects['Cube'].active_shape_key_index = -1
bpy.data.objects['Light'].scale = (bpy.data.objects['Cube'].active_shape_key_index + 1, 1, 1)
bpy.data.objects['Cube'].active_shape_key.name
