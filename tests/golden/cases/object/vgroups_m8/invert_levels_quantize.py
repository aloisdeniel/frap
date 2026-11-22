# WP 8.3.7: `object.vertex_group_invert` (auto_assign / auto_remove, subsets), `vertex_group_levels`
# (offset, gain, clamping) and `vertex_group_quantize` (steps) in Object Mode and on the selected
# vertices in Edit Mode, with a locked group (these three operators do not honour locks, as in
# Blender), the polls and the `group_select_mode` items. Each stage is kept in a duplicate
# (Cube.001..Cube.003) and every mesh shows its weights through three DISPLACE modifiers (groups A,
# B, C along X, Y, Z, half the weight each); membership is read with `VertexGroup.weight()`.
# golden: startup=factory tier=T1 compare=objects,evaluated:Cube,evaluated:Cube.001,evaluated:Cube.002,evaluated:Cube.003
import bpy
bpy.ops.object.vertex_group_invert()
bpy.data.objects['Cube'].vertex_groups.new(name='A')
bpy.data.objects['Cube'].vertex_groups.new(name='B')
bpy.data.objects['Cube'].vertex_groups.new(name='C')
bpy.ops.object.vertex_group_levels(offset=0.5)
bpy.ops.object.vertex_group_invert(group_select_mode='BONE_DEFORM')
bpy.data.objects['Cube'].vertex_groups['A'].add([0, 1, 2, 3], 0.2, 'REPLACE')
bpy.data.objects['Cube'].vertex_groups['A'].add([4, 5], 0.75, 'REPLACE')
bpy.data.objects['Cube'].vertex_groups['B'].add([0, 2, 4, 6], 0.4, 'REPLACE')
bpy.data.objects['Cube'].vertex_groups['C'].add([1], 0.33, 'REPLACE')
bpy.data.objects['Cube'].vertex_groups['C'].add([7], 1.0, 'REPLACE')
bpy.ops.object.vertex_group_set_active(group='A')
bpy.ops.object.vertex_group_invert(auto_assign=False, auto_remove=False)
bpy.data.objects['Cube'].vertex_groups['A'].weight(6)
bpy.data.objects['Cube'].vertex_groups['B'].lock_weight = True
bpy.ops.object.vertex_group_invert(group_select_mode='ALL')
bpy.data.objects['Cube'].vertex_groups['C'].weight(7)
bpy.data.objects['Cube'].vertex_groups['B'].weight(1)
bpy.ops.object.duplicate()
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Cube'].select_set(True)
bpy.ops.object.vertex_group_levels(offset=0.1, gain=0.5)
bpy.ops.object.vertex_group_levels(group_select_mode='ALL', offset=0.25, gain=1.5)
bpy.ops.object.vertex_group_set_active(group='C')
bpy.ops.object.vertex_group_levels(offset=-0.9, gain=1.0)
bpy.ops.object.duplicate()
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Cube'].select_set(True)
bpy.ops.object.vertex_group_quantize(steps=3)
bpy.ops.object.vertex_group_quantize(group_select_mode='ALL', steps=2)
bpy.ops.object.vertex_group_quantize(group_select_mode='ACTIVE', steps=1)
bpy.ops.object.duplicate()
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Cube'].select_set(True)
bpy.data.objects['Cube'].vertex_groups['A'].add([0, 1, 2, 3, 4, 5, 6, 7], 0.3, 'REPLACE')
bpy.data.objects['Cube'].vertex_groups['B'].add([0, 1, 2, 3, 4, 5, 6, 7], 0.6, 'REPLACE')
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 3, 5])
bpy.ops.object.vertex_group_set_active(group='A')
bpy.ops.object.vertex_group_invert()
bpy.ops.object.vertex_group_levels(group_select_mode='ALL', offset=0.05, gain=1.2)
bpy.ops.object.vertex_group_quantize(steps=5)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Camera'].location = (bpy.data.objects['Cube'].vertex_groups['A'].weight(0), bpy.data.objects['Cube'].vertex_groups['A'].weight(1), bpy.data.objects['Cube'].vertex_groups['B'].weight(3))
bpy.data.objects['Camera'].scale = (bpy.data.objects['Cube'].vertex_groups['B'].lock_weight, bpy.data.objects['Cube'].vertex_groups.active_index, 1)
bpy.data.objects['Cube'].modifiers.new(name='A', type='DISPLACE')
bpy.data.objects['Cube'].modifiers['A'].vertex_group = 'A'
bpy.data.objects['Cube'].modifiers['A'].direction = 'X'
bpy.data.objects['Cube'].modifiers.new(name='B', type='DISPLACE')
bpy.data.objects['Cube'].modifiers['B'].vertex_group = 'B'
bpy.data.objects['Cube'].modifiers['B'].direction = 'Y'
bpy.data.objects['Cube'].modifiers.new(name='C', type='DISPLACE')
bpy.data.objects['Cube'].modifiers['C'].vertex_group = 'C'
bpy.data.objects['Cube'].modifiers['C'].direction = 'Z'
bpy.data.objects['Cube.001'].modifiers.new(name='A', type='DISPLACE')
bpy.data.objects['Cube.001'].modifiers['A'].vertex_group = 'A'
bpy.data.objects['Cube.001'].modifiers['A'].direction = 'X'
bpy.data.objects['Cube.001'].modifiers.new(name='B', type='DISPLACE')
bpy.data.objects['Cube.001'].modifiers['B'].vertex_group = 'B'
bpy.data.objects['Cube.001'].modifiers['B'].direction = 'Y'
bpy.data.objects['Cube.001'].modifiers.new(name='C', type='DISPLACE')
bpy.data.objects['Cube.001'].modifiers['C'].vertex_group = 'C'
bpy.data.objects['Cube.001'].modifiers['C'].direction = 'Z'
bpy.data.objects['Cube.002'].modifiers.new(name='A', type='DISPLACE')
bpy.data.objects['Cube.002'].modifiers['A'].vertex_group = 'A'
bpy.data.objects['Cube.002'].modifiers['A'].direction = 'X'
bpy.data.objects['Cube.002'].modifiers.new(name='B', type='DISPLACE')
bpy.data.objects['Cube.002'].modifiers['B'].vertex_group = 'B'
bpy.data.objects['Cube.002'].modifiers['B'].direction = 'Y'
bpy.data.objects['Cube.002'].modifiers.new(name='C', type='DISPLACE')
bpy.data.objects['Cube.002'].modifiers['C'].vertex_group = 'C'
bpy.data.objects['Cube.002'].modifiers['C'].direction = 'Z'
bpy.data.objects['Cube.003'].modifiers.new(name='A', type='DISPLACE')
bpy.data.objects['Cube.003'].modifiers['A'].vertex_group = 'A'
bpy.data.objects['Cube.003'].modifiers['A'].direction = 'X'
bpy.data.objects['Cube.003'].modifiers.new(name='B', type='DISPLACE')
bpy.data.objects['Cube.003'].modifiers['B'].vertex_group = 'B'
bpy.data.objects['Cube.003'].modifiers['B'].direction = 'Y'
bpy.data.objects['Cube.003'].modifiers.new(name='C', type='DISPLACE')
bpy.data.objects['Cube.003'].modifiers['C'].vertex_group = 'C'
bpy.data.objects['Cube.003'].modifiers['C'].direction = 'Z'
