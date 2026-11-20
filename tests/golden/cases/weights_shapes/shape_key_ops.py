# WP 6.3: the shape-key operators and RNA functions. `object.shape_key_add` (Basis / Key N,
# value 1, from the mix), `shape_key_move` (TOP/UP/DOWN/BOTTOM with relative keys following),
# `shape_key_clear` (slider ranges), `shape_key_remove` (selected + active, locked ones kept,
# the basis replaced by the next block in the mesh), `Object.shape_key_remove` / `_clear`,
# `ShapeKey.name` uniqueness, `Mesh.transform(shape_keys=True)` and the polls. Blocks are
# compared through linked duplicates showing single blocks and the mesh itself.
# golden: startup=factory tier=T1 compare=objects,mesh:Cube,evaluated:Cube,evaluated:Cube.001,evaluated:Cube.002
import bpy
bpy.ops.object.duplicate(linked=True)
bpy.ops.object.duplicate(linked=True)
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Cube'].select_set(True)
bpy.ops.object.shape_key_remove()
bpy.ops.object.shape_key_move(type='UP')
bpy.ops.object.shape_key_clear()
bpy.ops.object.shape_key_add(from_mix=False)
bpy.ops.object.shape_key_add(from_mix=False)
bpy.ops.object.shape_key_add(from_mix=False)
bpy.ops.object.shape_key_add(from_mix=True)
bpy.data.meshes['Cube'].shape_keys.key_blocks.keys()
#@blender assert bpy.data.meshes['Cube'].shape_keys.key_blocks.keys() == ['Basis', 'Key 1', 'Key 2', 'Key 3']
bpy.data.objects['Cube'].active_shape_key_index
bpy.data.meshes['Cube'].shape_keys.key_blocks['Key 1'].value
bpy.data.meshes['Cube'].shape_keys.key_blocks['Key 1'].data.foreach_set('co', [1.5, 1, 1, 1.5, 1, -1, 1.5, -1, 1, 1.5, -1, -1, -0.5, 1, 1, -0.5, 1, -1, -0.5, -1, 1, -0.5, -1, -1])
bpy.data.meshes['Cube'].shape_keys.key_blocks['Key 2'].data.foreach_set('co', [1, 1.5, 1, 1, 1.5, -1, 1, -0.5, 1, 1, -0.5, -1, -1, 1.5, 1, -1, 1.5, -1, -1, -0.5, 1, -1, -0.5, -1])
bpy.data.meshes['Cube'].shape_keys.key_blocks['Key 3'].relative_key = bpy.data.meshes['Cube'].shape_keys.key_blocks['Key 1']
bpy.data.meshes['Cube'].shape_keys.key_blocks['Key 3'].name = 'Key 1'
#@blender assert bpy.data.meshes['Cube'].shape_keys.key_blocks[3].name == 'Key 1.001'
bpy.data.meshes['Cube'].shape_keys.key_blocks['Key 1.001'].name = 'Mix'
bpy.ops.object.shape_key_add(from_mix=True)
bpy.data.meshes['Cube'].shape_keys.key_blocks['Key 4'].data[0].co
bpy.data.objects['Cube'].active_shape_key_index = 2
bpy.ops.object.shape_key_move(type='TOP')
bpy.data.meshes['Cube'].shape_keys.key_blocks.keys()
#@blender assert bpy.data.meshes['Cube'].shape_keys.key_blocks.keys() == ['Basis', 'Key 2', 'Key 1', 'Mix', 'Key 4']
#@blender assert bpy.data.meshes['Cube'].shape_keys.key_blocks['Mix'].relative_key.name == 'Key 1'
#@blender assert bpy.data.objects['Cube'].active_shape_key_index == 1
bpy.data.meshes['Cube'].shape_keys.key_blocks['Key 4'].select = True
bpy.ops.object.shape_key_move(type='DOWN')
bpy.ops.object.shape_key_move(type='BOTTOM')
bpy.ops.object.shape_key_move(type='UP')
bpy.data.meshes['Cube'].shape_keys.key_blocks.keys()
bpy.data.objects['Cube'].active_shape_key_index
bpy.data.meshes['Cube'].shape_keys.key_blocks['Key 4'].select = False
bpy.data.meshes['Cube'].shape_keys.key_blocks['Key 2'].slider_min = 0.5
bpy.ops.object.shape_key_clear()
bpy.data.objects['Camera'].location = (bpy.data.meshes['Cube'].shape_keys.key_blocks['Key 2'].value, bpy.data.meshes['Cube'].shape_keys.key_blocks['Key 1'].value, bpy.data.objects['Cube'].active_shape_key_index)
bpy.data.meshes['Cube'].shape_keys.key_blocks['Key 1'].value = 1.0
bpy.data.meshes['Cube'].shape_keys.key_blocks['Mix'].value = 0.5
bpy.data.meshes['Cube'].shape_keys.key_blocks['Mix'].lock_shape = True
bpy.data.meshes['Cube'].shape_keys.key_blocks['Mix'].select = True
bpy.ops.object.shape_key_remove(all=True)
bpy.ops.object.shape_key_remove()
bpy.data.meshes['Cube'].shape_keys.key_blocks.keys()
bpy.data.objects['Cube'].active_shape_key_index = 0
bpy.data.meshes['Cube'].shape_keys.key_blocks['Mix'].select = False
bpy.ops.object.shape_key_remove()
bpy.data.meshes['Cube'].shape_keys.key_blocks.keys()
bpy.data.objects['Cube'].active_shape_key_index
bpy.data.meshes['Cube'].shape_keys.key_blocks['Mix'].lock_shape = False
bpy.data.objects['Cube'].shape_key_remove(bpy.data.meshes['Cube'].shape_keys.key_blocks['Key 4'])
bpy.data.objects['Camera'].shape_key_add()
bpy.data.objects['Camera'].shape_key_remove(bpy.data.meshes['Cube'].shape_keys.key_blocks['Mix'])
bpy.data.meshes['Cube'].transform(((2, 0, 0, 0), (0, 1, 0, 0), (0, 0, 1, 0), (0, 0, 0, 1)), shape_keys=True)
bpy.data.meshes['Cube'].transform(((1, 0, 0, 0), (0, 1, 0, 0), (0, 0, 1, 1), (0, 0, 0, 1)))
bpy.data.objects['Cube.001'].show_only_shape_key = True
bpy.data.objects['Cube.001'].active_shape_key_index = 1
bpy.data.objects['Cube.002'].show_only_shape_key = True
bpy.data.objects['Cube.002'].active_shape_key_index = 2
bpy.data.meshes['Cube'].shape_keys.key_blocks.keys()
bpy.data.objects['Cube.002'].shape_key_clear()
bpy.data.meshes['Cube'].shape_keys
