# WP 6.3: shape keys through `object.join` (the active key keeps its blocks, gains the blocks
# only joined meshes have with relative keys remapped by name, joined parts contribute their
# block of the same name or their positions, transformed into the active object's space) and
# `mesh.separate` (Edit Mode: the parts' meshes get a copy of the key with their layers;
# Object Mode LOOSE: the remaining key is rewritten too).
# golden: startup=factory tier=T1 compare=mesh:Cube,mesh:Cube.003,mesh:Cube.004,evaluated:Cube,evaluated:Cube.001,evaluated:Cube.002
import bpy
bpy.ops.object.duplicate()
bpy.ops.object.duplicate()
bpy.data.objects['Cube.001'].location = (4, 0, 0)
bpy.data.objects['Cube.002'].location = (0, 4, 0)
bpy.data.objects['Cube.002'].scale = (0.5, 0.5, 0.5)
bpy.data.objects['Cube'].shape_key_add(name='Basis')
bpy.data.objects['Cube'].shape_key_add(name='Up', from_mix=False)
bpy.data.shape_keys['Key'].key_blocks['Up'].data.foreach_set('co', [1, 1, 2, 1, 1, 0, 1, -1, 2, 1, -1, 0, -1, 1, 2, -1, 1, 0, -1, -1, 2, -1, -1, 0])
bpy.data.objects['Cube.001'].shape_key_add(name='Basis')
bpy.data.objects['Cube.001'].shape_key_add(name='Side', from_mix=False)
bpy.data.objects['Cube.001'].shape_key_add(name='Up', from_mix=False)
bpy.data.meshes['Cube.001'].shape_keys.key_blocks['Side'].data.foreach_set('co', [2, 1, 1, 2, 1, -1, 2, -1, 1, 2, -1, -1, 0, 1, 1, 0, 1, -1, 0, -1, 1, 0, -1, -1])
bpy.data.meshes['Cube.001'].shape_keys.key_blocks['Up'].data.foreach_set('co', [1, 1, 3, 1, 1, -1, 1, -1, 3, 1, -1, -1, -1, 1, 3, -1, 1, -1, -1, -1, 3, -1, -1, -1])
bpy.data.meshes['Cube.001'].shape_keys.key_blocks['Side'].relative_key = bpy.data.meshes['Cube.001'].shape_keys.key_blocks['Up']
bpy.data.meshes['Cube.001'].shape_keys.key_blocks['Side'].value = 0.25
bpy.ops.object.select_all(action='SELECT')
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.object.join()
bpy.data.meshes['Cube'].shape_keys.key_blocks.keys()
#@blender assert bpy.data.meshes['Cube'].shape_keys.key_blocks.keys() == ['Basis', 'Up', 'Side']
#@blender assert bpy.data.meshes['Cube'].shape_keys.key_blocks['Side'].relative_key.name == 'Up'
bpy.data.meshes['Cube'].shape_keys.key_blocks['Up'].value = 1.0
bpy.data.meshes['Cube'].shape_keys.key_blocks['Side'].value = 1.0
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[8, 9, 10, 11, 12, 13, 14, 15])
bpy.ops.mesh.separate(type='SELECTED')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube.003'].shape_keys.key_blocks.keys()
bpy.data.meshes['Cube.003'].shape_keys.name
#@blender assert bpy.data.meshes['Cube.003'].shape_keys.name == 'Key.002'
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Cube'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.mesh.separate(type='LOOSE')
bpy.data.meshes['Cube'].shape_keys.key_blocks.keys()
bpy.data.meshes['Cube.004'].shape_keys.key_blocks.keys()
bpy.data.meshes['Cube.004'].shape_keys.key_blocks['Side'].value = 0.5
