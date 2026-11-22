# WP 8.3.7: `object.vertex_group_mirror` with the spatial X mirror: in Object Mode every pair
# swaps the active group's weights (all groups with `all_groups`) and flips `.L`/`.R` names (the
# active group and its partner only, or every group with `all_groups`); `mirror_weights` or
# `flip_group_names` alone; nothing without either. In Edit Mode a pair with one selected vertex
# copies the other one's weights, a fully selected pair swaps. A moved vertex has no mirror (the
# WARNING `N vertices mirrored, M failed`). Each stage is kept in a duplicate; weights are read
# through DISPLACE modifiers (G.L along X, G.R along Y, M along Z).
# golden: startup=factory tier=T1 compare=objects,evaluated:Cube,evaluated:Cube.001,evaluated:Cube.002,evaluated:Cube.003
import bpy
bpy.ops.object.vertex_group_mirror()
bpy.data.objects['Cube'].vertex_groups.new(name='G.L')
bpy.data.objects['Cube'].vertex_groups.new(name='G.R')
bpy.data.objects['Cube'].vertex_groups.new(name='M')
bpy.ops.object.vertex_group_mirror()
bpy.data.objects['Cube'].vertex_groups['G.L'].add([0], 0.25, 'REPLACE')
bpy.data.objects['Cube'].vertex_groups['G.L'].add([1], 0.5, 'REPLACE')
bpy.data.objects['Cube'].vertex_groups['G.R'].add([4], 0.75, 'REPLACE')
bpy.data.objects['Cube'].vertex_groups['G.R'].add([3], 0.125, 'REPLACE')
bpy.data.objects['Cube'].vertex_groups['M'].add([2], 0.6, 'REPLACE')
bpy.data.objects['Cube'].vertex_groups['M'].add([6], 0.1, 'REPLACE')
bpy.ops.object.vertex_group_set_active(group='G.L')
bpy.ops.object.vertex_group_mirror()
bpy.ops.object.duplicate()
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Cube'].select_set(True)
bpy.ops.object.vertex_group_mirror(all_groups=True)
bpy.ops.object.vertex_group_mirror(mirror_weights=False, flip_group_names=False)
bpy.ops.object.duplicate()
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Cube'].select_set(True)
bpy.ops.object.vertex_group_set_active(group='M')
bpy.ops.object.vertex_group_mirror(flip_group_names=False)
bpy.ops.object.vertex_group_set_active(group='G.R')
bpy.ops.object.vertex_group_mirror(mirror_weights=False, all_groups=True)
bpy.ops.object.duplicate()
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Cube'].select_set(True)
bpy.data.meshes['Cube'].vertices[7].co = (-1.0, -1.0, -1.5)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 5, 6, 2, 3])
bpy.ops.object.vertex_group_set_active(group='G.L')
bpy.ops.object.vertex_group_mirror(all_groups=True)
bpy.ops.object.vertex_group_set_active(group='M')
bpy.ops.object.vertex_group_mirror()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Camera'].location = (bpy.data.objects['Cube'].vertex_groups['G.L'].index, bpy.data.objects['Cube'].vertex_groups['G.R'].index, bpy.data.objects['Cube'].vertex_groups.active_index)
bpy.data.objects['Cube'].modifiers.new(name='A', type='DISPLACE')
bpy.data.objects['Cube'].modifiers['A'].vertex_group = 'G.L'
bpy.data.objects['Cube'].modifiers['A'].direction = 'X'
bpy.data.objects['Cube'].modifiers.new(name='B', type='DISPLACE')
bpy.data.objects['Cube'].modifiers['B'].vertex_group = 'G.R'
bpy.data.objects['Cube'].modifiers['B'].direction = 'Y'
bpy.data.objects['Cube'].modifiers.new(name='C', type='DISPLACE')
bpy.data.objects['Cube'].modifiers['C'].vertex_group = 'M'
bpy.data.objects['Cube'].modifiers['C'].direction = 'Z'
bpy.data.objects['Cube.001'].modifiers.new(name='A', type='DISPLACE')
bpy.data.objects['Cube.001'].modifiers['A'].vertex_group = 'G.L'
bpy.data.objects['Cube.001'].modifiers['A'].direction = 'X'
bpy.data.objects['Cube.001'].modifiers.new(name='B', type='DISPLACE')
bpy.data.objects['Cube.001'].modifiers['B'].vertex_group = 'G.R'
bpy.data.objects['Cube.001'].modifiers['B'].direction = 'Y'
bpy.data.objects['Cube.001'].modifiers.new(name='C', type='DISPLACE')
bpy.data.objects['Cube.001'].modifiers['C'].vertex_group = 'M'
bpy.data.objects['Cube.001'].modifiers['C'].direction = 'Z'
bpy.data.objects['Cube.002'].modifiers.new(name='A', type='DISPLACE')
bpy.data.objects['Cube.002'].modifiers['A'].vertex_group = 'G.L'
bpy.data.objects['Cube.002'].modifiers['A'].direction = 'X'
bpy.data.objects['Cube.002'].modifiers.new(name='B', type='DISPLACE')
bpy.data.objects['Cube.002'].modifiers['B'].vertex_group = 'G.R'
bpy.data.objects['Cube.002'].modifiers['B'].direction = 'Y'
bpy.data.objects['Cube.002'].modifiers.new(name='C', type='DISPLACE')
bpy.data.objects['Cube.002'].modifiers['C'].vertex_group = 'M'
bpy.data.objects['Cube.002'].modifiers['C'].direction = 'Z'
bpy.data.objects['Cube.003'].modifiers.new(name='A', type='DISPLACE')
bpy.data.objects['Cube.003'].modifiers['A'].vertex_group = 'G.L'
bpy.data.objects['Cube.003'].modifiers['A'].direction = 'X'
bpy.data.objects['Cube.003'].modifiers.new(name='B', type='DISPLACE')
bpy.data.objects['Cube.003'].modifiers['B'].vertex_group = 'G.R'
bpy.data.objects['Cube.003'].modifiers['B'].direction = 'Y'
bpy.data.objects['Cube.003'].modifiers.new(name='C', type='DISPLACE')
bpy.data.objects['Cube.003'].modifiers['C'].vertex_group = 'M'
bpy.data.objects['Cube.003'].modifiers['C'].direction = 'Z'
