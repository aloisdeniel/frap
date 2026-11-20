# WP 6.3: absolute shape keys (`use_relative = False`): blocks sit at `frame = pos × 100`
# (0, 10, 20, … as added), the mesh is interpolated between the blocks around `eval_time`
# (`get_keys_for_absolute_eval`, `key_curve_position_weights`) with the blocks' interpolation
# (linear, cardinal, Catmull-Rom, B-spline), clamped at the ends.
# golden: startup=factory tier=T1 tol=1e-5 compare=evaluated:Cube,evaluated:Cube.001,evaluated:Cube.002,evaluated:Cube.003
import bpy
bpy.ops.object.duplicate()
bpy.ops.object.duplicate()
bpy.ops.object.duplicate()
bpy.data.objects['Cube'].shape_key_add(name='K0')
bpy.data.objects['Cube'].shape_key_add(name='K1', from_mix=False)
bpy.data.objects['Cube'].shape_key_add(name='K2', from_mix=False)
bpy.data.objects['Cube'].shape_key_add(name='K3', from_mix=False)
bpy.data.meshes['Cube'].shape_keys.key_blocks['K1'].data.foreach_set('co', [1, 1, 2, 1, 1, 0, 1, -1, 2, 1, -1, 0, -1, 1, 2, -1, 1, 0, -1, -1, 2, -1, -1, 0])
bpy.data.meshes['Cube'].shape_keys.key_blocks['K2'].data.foreach_set('co', [3, 1, 1, 3, 1, -1, 3, -1, 1, 3, -1, -1, 1, 1, 1, 1, 1, -1, 1, -1, 1, 1, -1, -1])
bpy.data.meshes['Cube'].shape_keys.key_blocks['K3'].data.foreach_set('co', [1, 4, 1, 1, 4, -1, 1, 2, 1, 1, 2, -1, -1, 4, 1, -1, 4, -1, -1, 2, 1, -1, 2, -1])
bpy.data.meshes['Cube'].shape_keys.use_relative = False
bpy.data.meshes['Cube'].shape_keys.eval_time = 15.0
bpy.data.meshes['Cube'].shape_keys.key_blocks['K3'].frame
bpy.data.meshes['Cube.001'].vertices[0].co = (1, 1, 1)
bpy.data.objects['Cube.001'].shape_key_add(name='K0')
bpy.data.objects['Cube.001'].shape_key_add(name='K1', from_mix=False)
bpy.data.objects['Cube.001'].shape_key_add(name='K2', from_mix=False)
bpy.data.objects['Cube.001'].shape_key_add(name='K3', from_mix=False)
bpy.data.meshes['Cube.001'].shape_keys.key_blocks['K1'].data.foreach_set('co', [1, 1, 2, 1, 1, 0, 1, -1, 2, 1, -1, 0, -1, 1, 2, -1, 1, 0, -1, -1, 2, -1, -1, 0])
bpy.data.meshes['Cube.001'].shape_keys.key_blocks['K2'].data.foreach_set('co', [3, 1, 1, 3, 1, -1, 3, -1, 1, 3, -1, -1, 1, 1, 1, 1, 1, -1, 1, -1, 1, 1, -1, -1])
bpy.data.meshes['Cube.001'].shape_keys.key_blocks['K3'].data.foreach_set('co', [1, 4, 1, 1, 4, -1, 1, 2, 1, 1, 2, -1, -1, 4, 1, -1, 4, -1, -1, 2, 1, -1, 2, -1])
bpy.data.meshes['Cube.001'].shape_keys.use_relative = False
bpy.data.meshes['Cube.001'].shape_keys.key_blocks['K1'].interpolation = 'KEY_CARDINAL'
bpy.data.meshes['Cube.001'].shape_keys.key_blocks['K2'].interpolation = 'KEY_CARDINAL'
bpy.data.meshes['Cube.001'].shape_keys.eval_time = 17.5
bpy.data.objects['Cube.002'].shape_key_add(name='K0')
bpy.data.objects['Cube.002'].shape_key_add(name='K1', from_mix=False)
bpy.data.objects['Cube.002'].shape_key_add(name='K2', from_mix=False)
bpy.data.objects['Cube.002'].shape_key_add(name='K3', from_mix=False)
bpy.data.meshes['Cube.002'].shape_keys.key_blocks['K1'].data.foreach_set('co', [1, 1, 2, 1, 1, 0, 1, -1, 2, 1, -1, 0, -1, 1, 2, -1, 1, 0, -1, -1, 2, -1, -1, 0])
bpy.data.meshes['Cube.002'].shape_keys.key_blocks['K2'].data.foreach_set('co', [3, 1, 1, 3, 1, -1, 3, -1, 1, 3, -1, -1, 1, 1, 1, 1, 1, -1, 1, -1, 1, 1, -1, -1])
bpy.data.meshes['Cube.002'].shape_keys.key_blocks['K3'].data.foreach_set('co', [1, 4, 1, 1, 4, -1, 1, 2, 1, 1, 2, -1, -1, 4, 1, -1, 4, -1, -1, 2, 1, -1, 2, -1])
bpy.data.meshes['Cube.002'].shape_keys.use_relative = False
bpy.data.meshes['Cube.002'].shape_keys.key_blocks['K1'].interpolation = 'KEY_BSPLINE'
bpy.data.meshes['Cube.002'].shape_keys.key_blocks['K2'].interpolation = 'KEY_CATMULL_ROM'
bpy.data.meshes['Cube.002'].shape_keys.eval_time = 12.0
bpy.data.objects['Cube.003'].shape_key_add(name='K0')
bpy.data.objects['Cube.003'].shape_key_add(name='K1', from_mix=False)
bpy.data.meshes['Cube.003'].shape_keys.key_blocks['K1'].data.foreach_set('co', [1, 1, 2, 1, 1, 0, 1, -1, 2, 1, -1, 0, -1, 1, 2, -1, 1, 0, -1, -1, 2, -1, -1, 0])
bpy.data.meshes['Cube.003'].shape_keys.use_relative = False
bpy.data.meshes['Cube.003'].shape_keys.eval_time = 250.0
bpy.data.meshes['Cube.003'].shape_keys.eval_time
