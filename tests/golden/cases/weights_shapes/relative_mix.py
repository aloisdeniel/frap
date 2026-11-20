# WP 6.3: relative shape-key mixing (`BKE_key_evaluate_object`): values, slider ranges,
# relative keys other than the basis, mute, vertex-group weights, `show_only_shape_key` (the
# active block alone; a muted active block shows the basis), and `shape_key_add(from_mix=True)`
# copying the current mix. Three linked duplicates share the mesh and its key but show
# different blocks. The basis differs from the mesh positions, which the mix ignores.
# golden: startup=factory tier=T1 compare=mesh:Cube,evaluated:Cube,evaluated:Cube.001,evaluated:Cube.002,evaluated:Cube.003
import bpy
bpy.ops.object.duplicate(linked=True)
bpy.ops.object.duplicate(linked=True)
bpy.ops.object.duplicate(linked=True)
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.data.objects['Cube'].shape_key_add(name='Basis')
bpy.data.objects['Cube'].shape_key_add(name='A', from_mix=False)
bpy.data.objects['Cube'].shape_key_add(name='B', from_mix=False)
bpy.data.objects['Cube'].shape_key_add(name='C', from_mix=False)
bpy.data.shape_keys['Key'].key_blocks['Basis'].value
bpy.data.shape_keys['Key'].key_blocks['A'].value
bpy.data.shape_keys['Key'].key_blocks['A'].data.foreach_set('co', [1.5, 1, 1, 1.5, 1, -1, 1.5, -1, 1, 1.5, -1, -1, -0.5, 1, 1, -0.5, 1, -1, -0.5, -1, 1, -0.5, -1, -1])
bpy.data.shape_keys['Key'].key_blocks['B'].data.foreach_set('co', [1.5, 2, 1, 1.5, 2, -1, 1.5, 0, 1, 1.5, 0, -1, -0.5, 1, 1, -0.5, 1, -1, -0.5, -1, 1, -0.5, -1, -1])
bpy.data.shape_keys['Key'].key_blocks['C'].data.foreach_set('co', [2, 2, 2, 2, 2, -2, 2, -2, 2, 2, -2, -2, -2, 2, 2, -2, 2, -2, -2, -2, 2, -2, -2, -2])
bpy.data.shape_keys['Key'].key_blocks['B'].relative_key = bpy.data.shape_keys['Key'].key_blocks['A']
bpy.data.shape_keys['Key'].key_blocks['B'].relative_key = bpy.data.shape_keys['Key'].key_blocks['B']
#@blender assert bpy.data.shape_keys['Key'].key_blocks['B'].relative_key.name == 'B'
bpy.data.shape_keys['Key'].key_blocks['B'].relative_key = bpy.data.shape_keys['Key'].key_blocks['A']
bpy.data.shape_keys['Key'].key_blocks['A'].value = 0.5
bpy.data.shape_keys['Key'].key_blocks['B'].slider_min = -1.0
bpy.data.shape_keys['Key'].key_blocks['B'].value = -0.5
#@blender assert abs(bpy.data.shape_keys['Key'].key_blocks['B'].value + 0.5) < 1e-6
bpy.data.shape_keys['Key'].key_blocks['B'].slider_max = -2.0
#@blender assert abs(bpy.data.shape_keys['Key'].key_blocks['B'].slider_max + 0.999) < 1e-6
bpy.data.shape_keys['Key'].key_blocks['B'].slider_max = 1.0
bpy.data.shape_keys['Key'].key_blocks['B'].slider_min = 5.0
#@blender assert abs(bpy.data.shape_keys['Key'].key_blocks['B'].slider_min - 0.999) < 1e-6
bpy.data.shape_keys['Key'].key_blocks['B'].slider_min = -1.0
bpy.data.shape_keys['Key'].key_blocks['B'].value = -0.5
bpy.data.shape_keys['Key'].key_blocks['C'].value = 1.0
bpy.data.shape_keys['Key'].key_blocks['C'].mute = True
bpy.data.objects['Cube'].shape_key_add(name='Mix', from_mix=True)
bpy.data.shape_keys['Key'].key_blocks['Mix'].value = 0.0
bpy.data.shape_keys['Key'].key_blocks['Mix'].frame
bpy.data.shape_keys['Key'].reference_key.name
bpy.data.objects['Cube'].vertex_groups.new(name='Half')
bpy.data.objects['Cube'].vertex_groups['Half'].add([0, 1, 4, 5], 0.5, 'REPLACE')
bpy.data.shape_keys['Key'].key_blocks['A'].vertex_group = 'Half'
bpy.data.objects['Cube.001'].show_only_shape_key = True
bpy.data.objects['Cube.001'].active_shape_key_index = 4
bpy.data.objects['Cube.001'].active_shape_key_index
bpy.data.objects['Cube.002'].show_only_shape_key = True
bpy.data.objects['Cube.002'].active_shape_key_index = 3
bpy.data.objects['Cube.003'].show_only_shape_key = True
bpy.data.objects['Cube.003'].active_shape_key_index = 1
bpy.data.objects['Cube.003'].active_shape_key_index = 99
#@blender assert bpy.data.objects['Cube.003'].active_shape_key_index == 99
#@blender assert bpy.data.objects['Cube.003'].active_shape_key is None
bpy.data.objects['Cube.003'].active_shape_key_index = 1
bpy.data.objects['Cube.003'].active_shape_key.name
bpy.data.meshes['Cube'].vertices[0].co = (3, 3, 3)
