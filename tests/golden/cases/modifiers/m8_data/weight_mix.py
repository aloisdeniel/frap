# WP 8.3.13: VERTEX_WEIGHT_MIX: every mix_mode on the AND set, SET and ADD on every
# mix_set, inverted groups, normalize, mask_constant, a mask group, without and with a
# missing group B. A second VERTEX_WEIGHT_EDIT adds the vertices left outside G at 0.333
# and DISPLACE shows the weights along Z.
# golden: startup=empty compare=evaluated:MSETALL,evaluated:MSETA,evaluated:MSETB,evaluated:MSETOR,evaluated:MSETAND,evaluated:MADDALL,evaluated:MADDA,evaluated:MADDB,evaluated:MADDOR,evaluated:MADDAND,evaluated:MSUBAND,evaluated:MMULAND,evaluated:MDIVAND,evaluated:MDIFAND,evaluated:MAVGAND,evaluated:MMINAND,evaluated:MMAXAND,evaluated:XInvA,evaluated:XInvB,evaluated:XNorm,evaluated:XInfluence,evaluated:XMaskGroup,evaluated:XNoB tier=T2
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=7, y_subdivisions=7, size=2)
bpy.context.object.name = 'MSETALL'
bpy.data.objects['MSETALL'].vertex_groups.new(name='G')
bpy.data.objects['MSETALL'].vertex_groups['G'].add([25, 54], 0.1, 'REPLACE')
bpy.data.objects['MSETALL'].vertex_groups['G'].add([4, 12, 24, 27, 32, 33, 41, 55, 58], 0.2, 'REPLACE')
bpy.data.objects['MSETALL'].vertex_groups['G'].add([9, 16, 17, 30, 37, 53, 59], 0.3, 'REPLACE')
bpy.data.objects['MSETALL'].vertex_groups['G'].add([57], 0.4, 'REPLACE')
bpy.data.objects['MSETALL'].vertex_groups['G'].add([35, 48, 62], 0.5, 'REPLACE')
bpy.data.objects['MSETALL'].vertex_groups['G'].add([8, 14, 18, 38, 44, 56], 0.6, 'REPLACE')
bpy.data.objects['MSETALL'].vertex_groups['G'].add([15, 36, 43, 45, 49, 52], 0.7, 'REPLACE')
bpy.data.objects['MSETALL'].vertex_groups['G'].add([0, 23], 0.8, 'REPLACE')
bpy.data.objects['MSETALL'].vertex_groups['G'].add([3, 5, 6, 40, 47], 0.9, 'REPLACE')
bpy.data.objects['MSETALL'].vertex_groups['G'].add([7, 51], 1.0, 'REPLACE')
bpy.data.objects['MSETALL'].vertex_groups.new(name='B')
bpy.data.objects['MSETALL'].vertex_groups['B'].add([1, 11, 14, 19, 24, 49, 53, 57], 0.1, 'REPLACE')
bpy.data.objects['MSETALL'].vertex_groups['B'].add([3, 18, 26], 0.2, 'REPLACE')
bpy.data.objects['MSETALL'].vertex_groups['B'].add([12, 17, 28, 35, 41, 46, 59], 0.3, 'REPLACE')
bpy.data.objects['MSETALL'].vertex_groups['B'].add([34, 42, 43, 45, 60], 0.4, 'REPLACE')
bpy.data.objects['MSETALL'].vertex_groups['B'].add([25, 33, 61], 0.5, 'REPLACE')
bpy.data.objects['MSETALL'].vertex_groups['B'].add([27, 29, 56], 0.6, 'REPLACE')
bpy.data.objects['MSETALL'].vertex_groups['B'].add([0, 2, 8, 30, 32, 40, 55], 0.7, 'REPLACE')
bpy.data.objects['MSETALL'].vertex_groups['B'].add([47, 54, 58, 62], 0.8, 'REPLACE')
bpy.data.objects['MSETALL'].vertex_groups['B'].add([50, 52, 63], 0.9, 'REPLACE')
bpy.data.objects['MSETALL'].vertex_groups['B'].add([5, 36, 39, 48], 1.0, 'REPLACE')
bpy.data.objects['MSETALL'].modifiers.new(name='W', type='VERTEX_WEIGHT_MIX')
bpy.data.objects['MSETALL'].modifiers['W'].vertex_group_a = 'G'
bpy.data.objects['MSETALL'].modifiers['W'].vertex_group_b = 'B'
bpy.data.objects['MSETALL'].modifiers['W'].mix_mode = 'SET'
bpy.data.objects['MSETALL'].modifiers['W'].mix_set = 'ALL'
bpy.data.objects['MSETALL'].modifiers['W'].default_weight_a = 0.2
bpy.data.objects['MSETALL'].modifiers['W'].default_weight_b = 0.6
bpy.data.objects['MSETALL'].modifiers.new(name='Probe', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['MSETALL'].modifiers['Probe'].vertex_group = 'G'
bpy.data.objects['MSETALL'].modifiers['Probe'].use_add = True
bpy.data.objects['MSETALL'].modifiers['Probe'].add_threshold = 0.0
bpy.data.objects['MSETALL'].modifiers['Probe'].default_weight = 0.333
bpy.data.objects['MSETALL'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['MSETALL'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['MSETALL'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['MSETALL'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['MSETALL'].modifiers['Dz'].strength = 1.0
bpy.ops.mesh.primitive_grid_add(x_subdivisions=7, y_subdivisions=7, size=2)
bpy.context.object.name = 'MSETA'
bpy.data.objects['MSETA'].vertex_groups.new(name='G')
bpy.data.objects['MSETA'].vertex_groups['G'].add([8, 41, 60], 0.1, 'REPLACE')
bpy.data.objects['MSETA'].vertex_groups['G'].add([1, 31], 0.2, 'REPLACE')
bpy.data.objects['MSETA'].vertex_groups['G'].add([18, 28, 29, 33, 49, 56], 0.3, 'REPLACE')
bpy.data.objects['MSETA'].vertex_groups['G'].add([2, 7, 17, 27, 38, 51], 0.4, 'REPLACE')
bpy.data.objects['MSETA'].vertex_groups['G'].add([12, 22, 54, 57], 0.6, 'REPLACE')
bpy.data.objects['MSETA'].vertex_groups['G'].add([4, 19, 25, 53], 0.7, 'REPLACE')
bpy.data.objects['MSETA'].vertex_groups['G'].add([6, 9, 10, 11], 0.8, 'REPLACE')
bpy.data.objects['MSETA'].vertex_groups['G'].add([0, 39, 43, 61], 0.9, 'REPLACE')
bpy.data.objects['MSETA'].vertex_groups['G'].add([5, 21, 26, 30, 37, 45, 48, 62, 63], 1.0, 'REPLACE')
bpy.data.objects['MSETA'].vertex_groups.new(name='B')
bpy.data.objects['MSETA'].vertex_groups['B'].add([16, 32, 35, 37, 45, 46, 61], 0.1, 'REPLACE')
bpy.data.objects['MSETA'].vertex_groups['B'].add([5, 10, 31, 38, 47], 0.2, 'REPLACE')
bpy.data.objects['MSETA'].vertex_groups['B'].add([3, 43, 55, 57, 59], 0.3, 'REPLACE')
bpy.data.objects['MSETA'].vertex_groups['B'].add([17, 49, 60, 62], 0.4, 'REPLACE')
bpy.data.objects['MSETA'].vertex_groups['B'].add([36], 0.5, 'REPLACE')
bpy.data.objects['MSETA'].vertex_groups['B'].add([1, 13, 19, 21, 27, 63], 0.6, 'REPLACE')
bpy.data.objects['MSETA'].vertex_groups['B'].add([12, 51, 52], 0.7, 'REPLACE')
bpy.data.objects['MSETA'].vertex_groups['B'].add([33, 34, 41, 50], 0.8, 'REPLACE')
bpy.data.objects['MSETA'].vertex_groups['B'].add([4, 14, 22], 0.9, 'REPLACE')
bpy.data.objects['MSETA'].vertex_groups['B'].add([8, 11, 18, 23, 30, 44], 1.0, 'REPLACE')
bpy.data.objects['MSETA'].modifiers.new(name='W', type='VERTEX_WEIGHT_MIX')
bpy.data.objects['MSETA'].modifiers['W'].vertex_group_a = 'G'
bpy.data.objects['MSETA'].modifiers['W'].vertex_group_b = 'B'
bpy.data.objects['MSETA'].modifiers['W'].mix_mode = 'SET'
bpy.data.objects['MSETA'].modifiers['W'].mix_set = 'A'
bpy.data.objects['MSETA'].modifiers['W'].default_weight_a = 0.2
bpy.data.objects['MSETA'].modifiers['W'].default_weight_b = 0.6
bpy.data.objects['MSETA'].modifiers.new(name='Probe', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['MSETA'].modifiers['Probe'].vertex_group = 'G'
bpy.data.objects['MSETA'].modifiers['Probe'].use_add = True
bpy.data.objects['MSETA'].modifiers['Probe'].add_threshold = 0.0
bpy.data.objects['MSETA'].modifiers['Probe'].default_weight = 0.333
bpy.data.objects['MSETA'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['MSETA'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['MSETA'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['MSETA'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['MSETA'].modifiers['Dz'].strength = 1.0
bpy.ops.mesh.primitive_grid_add(x_subdivisions=7, y_subdivisions=7, size=2)
bpy.context.object.name = 'MSETB'
bpy.data.objects['MSETB'].vertex_groups.new(name='G')
bpy.data.objects['MSETB'].vertex_groups['G'].add([6, 12, 19, 29], 0.1, 'REPLACE')
bpy.data.objects['MSETB'].vertex_groups['G'].add([39], 0.3, 'REPLACE')
bpy.data.objects['MSETB'].vertex_groups['G'].add([58], 0.4, 'REPLACE')
bpy.data.objects['MSETB'].vertex_groups['G'].add([0, 45, 59], 0.5, 'REPLACE')
bpy.data.objects['MSETB'].vertex_groups['G'].add([5, 46, 60], 0.6, 'REPLACE')
bpy.data.objects['MSETB'].vertex_groups['G'].add([24, 41, 44, 51, 61], 0.7, 'REPLACE')
bpy.data.objects['MSETB'].vertex_groups['G'].add([7, 11, 17, 27, 53, 55], 0.8, 'REPLACE')
bpy.data.objects['MSETB'].vertex_groups['G'].add([20, 23, 32, 43, 50], 0.9, 'REPLACE')
bpy.data.objects['MSETB'].vertex_groups['G'].add([8, 9, 10, 18, 52, 63], 1.0, 'REPLACE')
bpy.data.objects['MSETB'].vertex_groups.new(name='B')
bpy.data.objects['MSETB'].vertex_groups['B'].add([33, 50, 54, 60], 0.1, 'REPLACE')
bpy.data.objects['MSETB'].vertex_groups['B'].add([19, 35, 46, 55], 0.2, 'REPLACE')
bpy.data.objects['MSETB'].vertex_groups['B'].add([7, 11, 22, 28], 0.3, 'REPLACE')
bpy.data.objects['MSETB'].vertex_groups['B'].add([1, 13, 15, 47], 0.4, 'REPLACE')
bpy.data.objects['MSETB'].vertex_groups['B'].add([9, 14, 20, 23, 26], 0.5, 'REPLACE')
bpy.data.objects['MSETB'].vertex_groups['B'].add([29, 41, 57, 59], 0.6, 'REPLACE')
bpy.data.objects['MSETB'].vertex_groups['B'].add([8, 12, 18, 24, 27, 30, 45, 49, 51, 58], 0.7, 'REPLACE')
bpy.data.objects['MSETB'].vertex_groups['B'].add([10, 39, 52], 0.8, 'REPLACE')
bpy.data.objects['MSETB'].vertex_groups['B'].add([31, 32], 0.9, 'REPLACE')
bpy.data.objects['MSETB'].vertex_groups['B'].add([6, 34], 1.0, 'REPLACE')
bpy.data.objects['MSETB'].modifiers.new(name='W', type='VERTEX_WEIGHT_MIX')
bpy.data.objects['MSETB'].modifiers['W'].vertex_group_a = 'G'
bpy.data.objects['MSETB'].modifiers['W'].vertex_group_b = 'B'
bpy.data.objects['MSETB'].modifiers['W'].mix_mode = 'SET'
bpy.data.objects['MSETB'].modifiers['W'].mix_set = 'B'
bpy.data.objects['MSETB'].modifiers['W'].default_weight_a = 0.2
bpy.data.objects['MSETB'].modifiers['W'].default_weight_b = 0.6
bpy.data.objects['MSETB'].modifiers.new(name='Probe', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['MSETB'].modifiers['Probe'].vertex_group = 'G'
bpy.data.objects['MSETB'].modifiers['Probe'].use_add = True
bpy.data.objects['MSETB'].modifiers['Probe'].add_threshold = 0.0
bpy.data.objects['MSETB'].modifiers['Probe'].default_weight = 0.333
bpy.data.objects['MSETB'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['MSETB'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['MSETB'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['MSETB'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['MSETB'].modifiers['Dz'].strength = 1.0
bpy.ops.mesh.primitive_grid_add(x_subdivisions=7, y_subdivisions=7, size=2)
bpy.context.object.name = 'MSETOR'
bpy.data.objects['MSETOR'].vertex_groups.new(name='G')
bpy.data.objects['MSETOR'].vertex_groups['G'].add([4, 22, 30, 63], 0.1, 'REPLACE')
bpy.data.objects['MSETOR'].vertex_groups['G'].add([7, 9, 27, 31], 0.2, 'REPLACE')
bpy.data.objects['MSETOR'].vertex_groups['G'].add([11, 17, 19], 0.3, 'REPLACE')
bpy.data.objects['MSETOR'].vertex_groups['G'].add([0, 10, 14, 18, 44, 49, 52], 0.4, 'REPLACE')
bpy.data.objects['MSETOR'].vertex_groups['G'].add([16, 21, 23, 25, 38, 47], 0.5, 'REPLACE')
bpy.data.objects['MSETOR'].vertex_groups['G'].add([15], 0.6, 'REPLACE')
bpy.data.objects['MSETOR'].vertex_groups['G'].add([5, 8, 45, 46, 60], 0.7, 'REPLACE')
bpy.data.objects['MSETOR'].vertex_groups['G'].add([28, 29, 35, 61], 0.8, 'REPLACE')
bpy.data.objects['MSETOR'].vertex_groups['G'].add([12, 26, 42, 57, 62], 0.9, 'REPLACE')
bpy.data.objects['MSETOR'].vertex_groups['G'].add([1, 39], 1.0, 'REPLACE')
bpy.data.objects['MSETOR'].vertex_groups.new(name='B')
bpy.data.objects['MSETOR'].vertex_groups['B'].add([11, 16, 19, 22], 0.1, 'REPLACE')
bpy.data.objects['MSETOR'].vertex_groups['B'].add([20, 39], 0.2, 'REPLACE')
bpy.data.objects['MSETOR'].vertex_groups['B'].add([26, 28, 29, 37, 38, 46], 0.3, 'REPLACE')
bpy.data.objects['MSETOR'].vertex_groups['B'].add([24, 25, 31, 61], 0.4, 'REPLACE')
bpy.data.objects['MSETOR'].vertex_groups['B'].add([0, 17, 21, 53], 0.5, 'REPLACE')
bpy.data.objects['MSETOR'].vertex_groups['B'].add([6, 7, 35, 41, 51], 0.6, 'REPLACE')
bpy.data.objects['MSETOR'].vertex_groups['B'].add([10, 34, 40, 47, 55, 56], 0.7, 'REPLACE')
bpy.data.objects['MSETOR'].vertex_groups['B'].add([4, 30, 52, 57, 63], 0.8, 'REPLACE')
bpy.data.objects['MSETOR'].vertex_groups['B'].add([14, 23, 36, 59, 60], 0.9, 'REPLACE')
bpy.data.objects['MSETOR'].vertex_groups['B'].add([1, 42, 43, 54], 1.0, 'REPLACE')
bpy.data.objects['MSETOR'].modifiers.new(name='W', type='VERTEX_WEIGHT_MIX')
bpy.data.objects['MSETOR'].modifiers['W'].vertex_group_a = 'G'
bpy.data.objects['MSETOR'].modifiers['W'].vertex_group_b = 'B'
bpy.data.objects['MSETOR'].modifiers['W'].mix_mode = 'SET'
bpy.data.objects['MSETOR'].modifiers['W'].mix_set = 'OR'
bpy.data.objects['MSETOR'].modifiers['W'].default_weight_a = 0.2
bpy.data.objects['MSETOR'].modifiers['W'].default_weight_b = 0.6
bpy.data.objects['MSETOR'].modifiers.new(name='Probe', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['MSETOR'].modifiers['Probe'].vertex_group = 'G'
bpy.data.objects['MSETOR'].modifiers['Probe'].use_add = True
bpy.data.objects['MSETOR'].modifiers['Probe'].add_threshold = 0.0
bpy.data.objects['MSETOR'].modifiers['Probe'].default_weight = 0.333
bpy.data.objects['MSETOR'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['MSETOR'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['MSETOR'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['MSETOR'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['MSETOR'].modifiers['Dz'].strength = 1.0
bpy.ops.mesh.primitive_grid_add(x_subdivisions=7, y_subdivisions=7, size=2)
bpy.context.object.name = 'MSETAND'
bpy.data.objects['MSETAND'].vertex_groups.new(name='G')
bpy.data.objects['MSETAND'].vertex_groups['G'].add([13, 16, 17, 34], 0.1, 'REPLACE')
bpy.data.objects['MSETAND'].vertex_groups['G'].add([4, 15, 26, 28, 40, 44], 0.2, 'REPLACE')
bpy.data.objects['MSETAND'].vertex_groups['G'].add([0, 23, 31], 0.3, 'REPLACE')
bpy.data.objects['MSETAND'].vertex_groups['G'].add([7, 9, 41, 48, 49, 58, 60], 0.4, 'REPLACE')
bpy.data.objects['MSETAND'].vertex_groups['G'].add([6, 25, 29, 62], 0.5, 'REPLACE')
bpy.data.objects['MSETAND'].vertex_groups['G'].add([1, 5, 8, 12, 21, 35, 57, 61], 0.6, 'REPLACE')
bpy.data.objects['MSETAND'].vertex_groups['G'].add([47, 54], 0.7, 'REPLACE')
bpy.data.objects['MSETAND'].vertex_groups['G'].add([59], 0.8, 'REPLACE')
bpy.data.objects['MSETAND'].vertex_groups['G'].add([2, 18, 20, 36], 0.9, 'REPLACE')
bpy.data.objects['MSETAND'].vertex_groups['G'].add([32, 45, 51, 56], 1.0, 'REPLACE')
bpy.data.objects['MSETAND'].vertex_groups.new(name='B')
bpy.data.objects['MSETAND'].vertex_groups['B'].add([6, 11, 13, 33, 34, 47], 0.1, 'REPLACE')
bpy.data.objects['MSETAND'].vertex_groups['B'].add([5, 61, 63], 0.2, 'REPLACE')
bpy.data.objects['MSETAND'].vertex_groups['B'].add([14, 41, 48, 54], 0.3, 'REPLACE')
bpy.data.objects['MSETAND'].vertex_groups['B'].add([21, 28, 43, 56], 0.4, 'REPLACE')
bpy.data.objects['MSETAND'].vertex_groups['B'].add([8, 12, 37, 42, 53], 0.5, 'REPLACE')
bpy.data.objects['MSETAND'].vertex_groups['B'].add([22, 38, 60], 0.6, 'REPLACE')
bpy.data.objects['MSETAND'].vertex_groups['B'].add([27, 50, 55, 62], 0.7, 'REPLACE')
bpy.data.objects['MSETAND'].vertex_groups['B'].add([7, 16], 0.8, 'REPLACE')
bpy.data.objects['MSETAND'].vertex_groups['B'].add([10, 45, 46], 0.9, 'REPLACE')
bpy.data.objects['MSETAND'].vertex_groups['B'].add([19, 20, 57], 1.0, 'REPLACE')
bpy.data.objects['MSETAND'].modifiers.new(name='W', type='VERTEX_WEIGHT_MIX')
bpy.data.objects['MSETAND'].modifiers['W'].vertex_group_a = 'G'
bpy.data.objects['MSETAND'].modifiers['W'].vertex_group_b = 'B'
bpy.data.objects['MSETAND'].modifiers['W'].mix_mode = 'SET'
bpy.data.objects['MSETAND'].modifiers['W'].mix_set = 'AND'
bpy.data.objects['MSETAND'].modifiers['W'].default_weight_a = 0.2
bpy.data.objects['MSETAND'].modifiers['W'].default_weight_b = 0.6
bpy.data.objects['MSETAND'].modifiers.new(name='Probe', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['MSETAND'].modifiers['Probe'].vertex_group = 'G'
bpy.data.objects['MSETAND'].modifiers['Probe'].use_add = True
bpy.data.objects['MSETAND'].modifiers['Probe'].add_threshold = 0.0
bpy.data.objects['MSETAND'].modifiers['Probe'].default_weight = 0.333
bpy.data.objects['MSETAND'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['MSETAND'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['MSETAND'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['MSETAND'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['MSETAND'].modifiers['Dz'].strength = 1.0
bpy.ops.mesh.primitive_grid_add(x_subdivisions=7, y_subdivisions=7, size=2)
bpy.context.object.name = 'MADDALL'
bpy.data.objects['MADDALL'].vertex_groups.new(name='G')
bpy.data.objects['MADDALL'].vertex_groups['G'].add([26, 47, 62], 0.1, 'REPLACE')
bpy.data.objects['MADDALL'].vertex_groups['G'].add([22], 0.2, 'REPLACE')
bpy.data.objects['MADDALL'].vertex_groups['G'].add([0, 9, 56, 63], 0.3, 'REPLACE')
bpy.data.objects['MADDALL'].vertex_groups['G'].add([13, 19, 29, 34, 35, 42, 48, 55], 0.4, 'REPLACE')
bpy.data.objects['MADDALL'].vertex_groups['G'].add([4, 5, 18, 59], 0.5, 'REPLACE')
bpy.data.objects['MADDALL'].vertex_groups['G'].add([3, 10, 17, 39, 44, 52], 0.6, 'REPLACE')
bpy.data.objects['MADDALL'].vertex_groups['G'].add([16, 24], 0.7, 'REPLACE')
bpy.data.objects['MADDALL'].vertex_groups['G'].add([7, 14, 27], 0.8, 'REPLACE')
bpy.data.objects['MADDALL'].vertex_groups['G'].add([2, 37, 61], 0.9, 'REPLACE')
bpy.data.objects['MADDALL'].vertex_groups['G'].add([23, 41, 54], 1.0, 'REPLACE')
bpy.data.objects['MADDALL'].vertex_groups.new(name='B')
bpy.data.objects['MADDALL'].vertex_groups['B'].add([25, 36, 49, 51], 0.1, 'REPLACE')
bpy.data.objects['MADDALL'].vertex_groups['B'].add([14, 22, 55, 63], 0.2, 'REPLACE')
bpy.data.objects['MADDALL'].vertex_groups['B'].add([9, 12, 15, 28, 44, 47, 54, 59, 61], 0.3, 'REPLACE')
bpy.data.objects['MADDALL'].vertex_groups['B'].add([13, 27, 33, 45, 57], 0.4, 'REPLACE')
bpy.data.objects['MADDALL'].vertex_groups['B'].add([31, 40], 0.5, 'REPLACE')
bpy.data.objects['MADDALL'].vertex_groups['B'].add([1, 18, 20, 58, 60], 0.6, 'REPLACE')
bpy.data.objects['MADDALL'].vertex_groups['B'].add([0, 2, 32, 41], 0.7, 'REPLACE')
bpy.data.objects['MADDALL'].vertex_groups['B'].add([7, 26, 35], 0.8, 'REPLACE')
bpy.data.objects['MADDALL'].vertex_groups['B'].add([10, 17, 24, 48, 52, 53], 0.9, 'REPLACE')
bpy.data.objects['MADDALL'].vertex_groups['B'].add([34], 1.0, 'REPLACE')
bpy.data.objects['MADDALL'].modifiers.new(name='W', type='VERTEX_WEIGHT_MIX')
bpy.data.objects['MADDALL'].modifiers['W'].vertex_group_a = 'G'
bpy.data.objects['MADDALL'].modifiers['W'].vertex_group_b = 'B'
bpy.data.objects['MADDALL'].modifiers['W'].mix_mode = 'ADD'
bpy.data.objects['MADDALL'].modifiers['W'].mix_set = 'ALL'
bpy.data.objects['MADDALL'].modifiers['W'].default_weight_a = 0.2
bpy.data.objects['MADDALL'].modifiers['W'].default_weight_b = 0.6
bpy.data.objects['MADDALL'].modifiers.new(name='Probe', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['MADDALL'].modifiers['Probe'].vertex_group = 'G'
bpy.data.objects['MADDALL'].modifiers['Probe'].use_add = True
bpy.data.objects['MADDALL'].modifiers['Probe'].add_threshold = 0.0
bpy.data.objects['MADDALL'].modifiers['Probe'].default_weight = 0.333
bpy.data.objects['MADDALL'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['MADDALL'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['MADDALL'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['MADDALL'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['MADDALL'].modifiers['Dz'].strength = 1.0
bpy.ops.mesh.primitive_grid_add(x_subdivisions=7, y_subdivisions=7, size=2)
bpy.context.object.name = 'MADDA'
bpy.data.objects['MADDA'].vertex_groups.new(name='G')
bpy.data.objects['MADDA'].vertex_groups['G'].add([7, 27, 28, 39, 60, 62], 0.1, 'REPLACE')
bpy.data.objects['MADDA'].vertex_groups['G'].add([15, 18, 49, 55], 0.2, 'REPLACE')
bpy.data.objects['MADDA'].vertex_groups['G'].add([26, 58, 59], 0.3, 'REPLACE')
bpy.data.objects['MADDA'].vertex_groups['G'].add([33, 43, 47], 0.4, 'REPLACE')
bpy.data.objects['MADDA'].vertex_groups['G'].add([37, 41], 0.5, 'REPLACE')
bpy.data.objects['MADDA'].vertex_groups['G'].add([10, 12, 23, 53], 0.6, 'REPLACE')
bpy.data.objects['MADDA'].vertex_groups['G'].add([2, 32, 38], 0.7, 'REPLACE')
bpy.data.objects['MADDA'].vertex_groups['G'].add([1, 17, 22, 35, 36, 50, 52, 63], 0.8, 'REPLACE')
bpy.data.objects['MADDA'].vertex_groups['G'].add([0, 3, 30, 46], 0.9, 'REPLACE')
bpy.data.objects['MADDA'].vertex_groups['G'].add([5, 13, 34, 57, 61], 1.0, 'REPLACE')
bpy.data.objects['MADDA'].vertex_groups.new(name='B')
bpy.data.objects['MADDA'].vertex_groups['B'].add([1, 13, 31, 45, 47], 0.2, 'REPLACE')
bpy.data.objects['MADDA'].vertex_groups['B'].add([5, 19, 58], 0.3, 'REPLACE')
bpy.data.objects['MADDA'].vertex_groups['B'].add([3, 7, 8, 23, 28, 29, 38, 41, 42, 54], 0.4, 'REPLACE')
bpy.data.objects['MADDA'].vertex_groups['B'].add([11, 18, 20, 22, 40, 55, 61], 0.5, 'REPLACE')
bpy.data.objects['MADDA'].vertex_groups['B'].add([12, 16, 21, 32, 37, 48], 0.6, 'REPLACE')
bpy.data.objects['MADDA'].vertex_groups['B'].add([35, 43], 0.7, 'REPLACE')
bpy.data.objects['MADDA'].vertex_groups['B'].add([4, 53], 0.8, 'REPLACE')
bpy.data.objects['MADDA'].vertex_groups['B'].add([17, 25, 39, 63], 0.9, 'REPLACE')
bpy.data.objects['MADDA'].vertex_groups['B'].add([6, 9, 10, 33, 34], 1.0, 'REPLACE')
bpy.data.objects['MADDA'].modifiers.new(name='W', type='VERTEX_WEIGHT_MIX')
bpy.data.objects['MADDA'].modifiers['W'].vertex_group_a = 'G'
bpy.data.objects['MADDA'].modifiers['W'].vertex_group_b = 'B'
bpy.data.objects['MADDA'].modifiers['W'].mix_mode = 'ADD'
bpy.data.objects['MADDA'].modifiers['W'].mix_set = 'A'
bpy.data.objects['MADDA'].modifiers['W'].default_weight_a = 0.2
bpy.data.objects['MADDA'].modifiers['W'].default_weight_b = 0.6
bpy.data.objects['MADDA'].modifiers.new(name='Probe', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['MADDA'].modifiers['Probe'].vertex_group = 'G'
bpy.data.objects['MADDA'].modifiers['Probe'].use_add = True
bpy.data.objects['MADDA'].modifiers['Probe'].add_threshold = 0.0
bpy.data.objects['MADDA'].modifiers['Probe'].default_weight = 0.333
bpy.data.objects['MADDA'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['MADDA'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['MADDA'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['MADDA'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['MADDA'].modifiers['Dz'].strength = 1.0
bpy.ops.mesh.primitive_grid_add(x_subdivisions=7, y_subdivisions=7, size=2)
bpy.context.object.name = 'MADDB'
bpy.data.objects['MADDB'].vertex_groups.new(name='G')
bpy.data.objects['MADDB'].vertex_groups['G'].add([20, 22, 30, 34, 46, 48], 0.1, 'REPLACE')
bpy.data.objects['MADDB'].vertex_groups['G'].add([17, 40, 44], 0.2, 'REPLACE')
bpy.data.objects['MADDB'].vertex_groups['G'].add([1, 33], 0.3, 'REPLACE')
bpy.data.objects['MADDB'].vertex_groups['G'].add([12, 47, 52], 0.4, 'REPLACE')
bpy.data.objects['MADDB'].vertex_groups['G'].add([2, 5, 15, 39, 51], 0.5, 'REPLACE')
bpy.data.objects['MADDB'].vertex_groups['G'].add([4, 31, 36, 42, 45, 54], 0.6, 'REPLACE')
bpy.data.objects['MADDB'].vertex_groups['G'].add([26, 27, 35, 55], 0.7, 'REPLACE')
bpy.data.objects['MADDB'].vertex_groups['G'].add([11, 38, 57], 0.8, 'REPLACE')
bpy.data.objects['MADDB'].vertex_groups['G'].add([7, 16, 21, 37, 50, 60, 63], 0.9, 'REPLACE')
bpy.data.objects['MADDB'].vertex_groups['G'].add([23, 58], 1.0, 'REPLACE')
bpy.data.objects['MADDB'].vertex_groups.new(name='B')
bpy.data.objects['MADDB'].vertex_groups['B'].add([11, 13, 28], 0.1, 'REPLACE')
bpy.data.objects['MADDB'].vertex_groups['B'].add([16, 25, 31, 36, 45, 47, 58], 0.2, 'REPLACE')
bpy.data.objects['MADDB'].vertex_groups['B'].add([7, 10, 12, 41], 0.3, 'REPLACE')
bpy.data.objects['MADDB'].vertex_groups['B'].add([14, 37, 50], 0.4, 'REPLACE')
bpy.data.objects['MADDB'].vertex_groups['B'].add([0, 15, 57], 0.5, 'REPLACE')
bpy.data.objects['MADDB'].vertex_groups['B'].add([6, 18, 48, 55], 0.6, 'REPLACE')
bpy.data.objects['MADDB'].vertex_groups['B'].add([43, 54, 56, 60], 0.7, 'REPLACE')
bpy.data.objects['MADDB'].vertex_groups['B'].add([20, 22, 38, 49, 53, 61], 0.8, 'REPLACE')
bpy.data.objects['MADDB'].vertex_groups['B'].add([23, 42, 62], 0.9, 'REPLACE')
bpy.data.objects['MADDB'].vertex_groups['B'].add([2, 5, 29, 32, 46, 63], 1.0, 'REPLACE')
bpy.data.objects['MADDB'].modifiers.new(name='W', type='VERTEX_WEIGHT_MIX')
bpy.data.objects['MADDB'].modifiers['W'].vertex_group_a = 'G'
bpy.data.objects['MADDB'].modifiers['W'].vertex_group_b = 'B'
bpy.data.objects['MADDB'].modifiers['W'].mix_mode = 'ADD'
bpy.data.objects['MADDB'].modifiers['W'].mix_set = 'B'
bpy.data.objects['MADDB'].modifiers['W'].default_weight_a = 0.2
bpy.data.objects['MADDB'].modifiers['W'].default_weight_b = 0.6
bpy.data.objects['MADDB'].modifiers.new(name='Probe', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['MADDB'].modifiers['Probe'].vertex_group = 'G'
bpy.data.objects['MADDB'].modifiers['Probe'].use_add = True
bpy.data.objects['MADDB'].modifiers['Probe'].add_threshold = 0.0
bpy.data.objects['MADDB'].modifiers['Probe'].default_weight = 0.333
bpy.data.objects['MADDB'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['MADDB'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['MADDB'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['MADDB'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['MADDB'].modifiers['Dz'].strength = 1.0
bpy.ops.mesh.primitive_grid_add(x_subdivisions=7, y_subdivisions=7, size=2)
bpy.context.object.name = 'MADDOR'
bpy.data.objects['MADDOR'].vertex_groups.new(name='G')
bpy.data.objects['MADDOR'].vertex_groups['G'].add([5, 10, 23, 36, 55], 0.1, 'REPLACE')
bpy.data.objects['MADDOR'].vertex_groups['G'].add([1, 11, 37, 46, 62], 0.2, 'REPLACE')
bpy.data.objects['MADDOR'].vertex_groups['G'].add([21, 38, 41], 0.3, 'REPLACE')
bpy.data.objects['MADDOR'].vertex_groups['G'].add([17, 31, 48, 57], 0.4, 'REPLACE')
bpy.data.objects['MADDOR'].vertex_groups['G'].add([14, 33, 42, 49, 63], 0.5, 'REPLACE')
bpy.data.objects['MADDOR'].vertex_groups['G'].add([25, 27, 44, 45, 54, 59], 0.6, 'REPLACE')
bpy.data.objects['MADDOR'].vertex_groups['G'].add([24, 26, 43, 51], 0.7, 'REPLACE')
bpy.data.objects['MADDOR'].vertex_groups['G'].add([18, 19, 34], 0.8, 'REPLACE')
bpy.data.objects['MADDOR'].vertex_groups['G'].add([8, 12, 32, 50], 0.9, 'REPLACE')
bpy.data.objects['MADDOR'].vertex_groups['G'].add([7, 20, 35, 39, 60], 1.0, 'REPLACE')
bpy.data.objects['MADDOR'].vertex_groups.new(name='B')
bpy.data.objects['MADDOR'].vertex_groups['B'].add([28, 40, 42, 50], 0.1, 'REPLACE')
bpy.data.objects['MADDOR'].vertex_groups['B'].add([11, 16, 41, 46], 0.2, 'REPLACE')
bpy.data.objects['MADDOR'].vertex_groups['B'].add([27, 33], 0.3, 'REPLACE')
bpy.data.objects['MADDOR'].vertex_groups['B'].add([1, 18, 19, 26, 55, 63], 0.4, 'REPLACE')
bpy.data.objects['MADDOR'].vertex_groups['B'].add([0, 3, 4, 6, 20, 23, 32, 44, 48, 54, 58], 0.5, 'REPLACE')
bpy.data.objects['MADDOR'].vertex_groups['B'].add([51], 0.6, 'REPLACE')
bpy.data.objects['MADDOR'].vertex_groups['B'].add([12, 57, 61], 0.7, 'REPLACE')
bpy.data.objects['MADDOR'].vertex_groups['B'].add([2, 8, 14, 53, 60, 62], 0.8, 'REPLACE')
bpy.data.objects['MADDOR'].vertex_groups['B'].add([13, 15, 25, 34, 36, 49], 0.9, 'REPLACE')
bpy.data.objects['MADDOR'].vertex_groups['B'].add([29, 30, 35, 43, 47, 56], 1.0, 'REPLACE')
bpy.data.objects['MADDOR'].modifiers.new(name='W', type='VERTEX_WEIGHT_MIX')
bpy.data.objects['MADDOR'].modifiers['W'].vertex_group_a = 'G'
bpy.data.objects['MADDOR'].modifiers['W'].vertex_group_b = 'B'
bpy.data.objects['MADDOR'].modifiers['W'].mix_mode = 'ADD'
bpy.data.objects['MADDOR'].modifiers['W'].mix_set = 'OR'
bpy.data.objects['MADDOR'].modifiers['W'].default_weight_a = 0.2
bpy.data.objects['MADDOR'].modifiers['W'].default_weight_b = 0.6
bpy.data.objects['MADDOR'].modifiers.new(name='Probe', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['MADDOR'].modifiers['Probe'].vertex_group = 'G'
bpy.data.objects['MADDOR'].modifiers['Probe'].use_add = True
bpy.data.objects['MADDOR'].modifiers['Probe'].add_threshold = 0.0
bpy.data.objects['MADDOR'].modifiers['Probe'].default_weight = 0.333
bpy.data.objects['MADDOR'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['MADDOR'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['MADDOR'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['MADDOR'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['MADDOR'].modifiers['Dz'].strength = 1.0
bpy.ops.mesh.primitive_grid_add(x_subdivisions=7, y_subdivisions=7, size=2)
bpy.context.object.name = 'MADDAND'
bpy.data.objects['MADDAND'].vertex_groups.new(name='G')
bpy.data.objects['MADDAND'].vertex_groups['G'].add([2, 54, 57], 0.1, 'REPLACE')
bpy.data.objects['MADDAND'].vertex_groups['G'].add([10, 39], 0.2, 'REPLACE')
bpy.data.objects['MADDAND'].vertex_groups['G'].add([8, 11, 26, 32, 35, 43, 50], 0.3, 'REPLACE')
bpy.data.objects['MADDAND'].vertex_groups['G'].add([44], 0.4, 'REPLACE')
bpy.data.objects['MADDAND'].vertex_groups['G'].add([23, 27, 49], 0.5, 'REPLACE')
bpy.data.objects['MADDAND'].vertex_groups['G'].add([6, 19, 36, 45, 63], 0.6, 'REPLACE')
bpy.data.objects['MADDAND'].vertex_groups['G'].add([0, 53, 59, 60, 61, 62], 0.7, 'REPLACE')
bpy.data.objects['MADDAND'].vertex_groups['G'].add([13, 25, 29, 31, 51], 0.8, 'REPLACE')
bpy.data.objects['MADDAND'].vertex_groups['G'].add([3, 9, 18, 40], 0.9, 'REPLACE')
bpy.data.objects['MADDAND'].vertex_groups['G'].add([12, 34, 37, 38], 1.0, 'REPLACE')
bpy.data.objects['MADDAND'].vertex_groups.new(name='B')
bpy.data.objects['MADDAND'].vertex_groups['B'].add([25, 47, 51], 0.1, 'REPLACE')
bpy.data.objects['MADDAND'].vertex_groups['B'].add([3, 11, 19, 32, 52, 55], 0.2, 'REPLACE')
bpy.data.objects['MADDAND'].vertex_groups['B'].add([40, 45, 46, 62], 0.3, 'REPLACE')
bpy.data.objects['MADDAND'].vertex_groups['B'].add([27, 35, 36, 41, 43], 0.4, 'REPLACE')
bpy.data.objects['MADDAND'].vertex_groups['B'].add([4, 5, 21, 30, 39, 49], 0.5, 'REPLACE')
bpy.data.objects['MADDAND'].vertex_groups['B'].add([0, 13, 28, 31, 57], 0.6, 'REPLACE')
bpy.data.objects['MADDAND'].vertex_groups['B'].add([18, 23, 37, 42, 48, 50, 56, 61], 0.7, 'REPLACE')
bpy.data.objects['MADDAND'].vertex_groups['B'].add([14, 20, 34, 44, 53], 0.8, 'REPLACE')
bpy.data.objects['MADDAND'].vertex_groups['B'].add([12, 59], 0.9, 'REPLACE')
bpy.data.objects['MADDAND'].vertex_groups['B'].add([15, 24, 29], 1.0, 'REPLACE')
bpy.data.objects['MADDAND'].modifiers.new(name='W', type='VERTEX_WEIGHT_MIX')
bpy.data.objects['MADDAND'].modifiers['W'].vertex_group_a = 'G'
bpy.data.objects['MADDAND'].modifiers['W'].vertex_group_b = 'B'
bpy.data.objects['MADDAND'].modifiers['W'].mix_mode = 'ADD'
bpy.data.objects['MADDAND'].modifiers['W'].mix_set = 'AND'
bpy.data.objects['MADDAND'].modifiers['W'].default_weight_a = 0.2
bpy.data.objects['MADDAND'].modifiers['W'].default_weight_b = 0.6
bpy.data.objects['MADDAND'].modifiers.new(name='Probe', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['MADDAND'].modifiers['Probe'].vertex_group = 'G'
bpy.data.objects['MADDAND'].modifiers['Probe'].use_add = True
bpy.data.objects['MADDAND'].modifiers['Probe'].add_threshold = 0.0
bpy.data.objects['MADDAND'].modifiers['Probe'].default_weight = 0.333
bpy.data.objects['MADDAND'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['MADDAND'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['MADDAND'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['MADDAND'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['MADDAND'].modifiers['Dz'].strength = 1.0
bpy.ops.mesh.primitive_grid_add(x_subdivisions=7, y_subdivisions=7, size=2)
bpy.context.object.name = 'MSUBAND'
bpy.data.objects['MSUBAND'].vertex_groups.new(name='G')
bpy.data.objects['MSUBAND'].vertex_groups['G'].add([6, 49], 0.1, 'REPLACE')
bpy.data.objects['MSUBAND'].vertex_groups['G'].add([20, 32, 44], 0.2, 'REPLACE')
bpy.data.objects['MSUBAND'].vertex_groups['G'].add([1, 10, 21, 38, 39, 46, 53], 0.3, 'REPLACE')
bpy.data.objects['MSUBAND'].vertex_groups['G'].add([7, 25, 34, 56, 62], 0.4, 'REPLACE')
bpy.data.objects['MSUBAND'].vertex_groups['G'].add([23, 37, 48, 60, 63], 0.5, 'REPLACE')
bpy.data.objects['MSUBAND'].vertex_groups['G'].add([15], 0.6, 'REPLACE')
bpy.data.objects['MSUBAND'].vertex_groups['G'].add([11, 40, 45, 52], 0.7, 'REPLACE')
bpy.data.objects['MSUBAND'].vertex_groups['G'].add([17, 24, 26, 43], 0.8, 'REPLACE')
bpy.data.objects['MSUBAND'].vertex_groups['G'].add([18, 50, 51, 55], 0.9, 'REPLACE')
bpy.data.objects['MSUBAND'].vertex_groups['G'].add([19, 31], 1.0, 'REPLACE')
bpy.data.objects['MSUBAND'].vertex_groups.new(name='B')
bpy.data.objects['MSUBAND'].vertex_groups['B'].add([24, 35, 54, 57], 0.1, 'REPLACE')
bpy.data.objects['MSUBAND'].vertex_groups['B'].add([8, 22, 31, 48, 55], 0.2, 'REPLACE')
bpy.data.objects['MSUBAND'].vertex_groups['B'].add([1, 44, 52, 53], 0.3, 'REPLACE')
bpy.data.objects['MSUBAND'].vertex_groups['B'].add([4, 7, 17, 62], 0.4, 'REPLACE')
bpy.data.objects['MSUBAND'].vertex_groups['B'].add([6, 13, 23, 51], 0.5, 'REPLACE')
bpy.data.objects['MSUBAND'].vertex_groups['B'].add([19, 21, 39, 56], 0.6, 'REPLACE')
bpy.data.objects['MSUBAND'].vertex_groups['B'].add([0, 11, 18, 34, 41, 50], 0.7, 'REPLACE')
bpy.data.objects['MSUBAND'].vertex_groups['B'].add([15, 27, 32, 46, 49], 0.8, 'REPLACE')
bpy.data.objects['MSUBAND'].vertex_groups['B'].add([10, 25, 37, 42, 45, 59], 0.9, 'REPLACE')
bpy.data.objects['MSUBAND'].vertex_groups['B'].add([3, 5, 12, 20, 26, 58], 1.0, 'REPLACE')
bpy.data.objects['MSUBAND'].modifiers.new(name='W', type='VERTEX_WEIGHT_MIX')
bpy.data.objects['MSUBAND'].modifiers['W'].vertex_group_a = 'G'
bpy.data.objects['MSUBAND'].modifiers['W'].vertex_group_b = 'B'
bpy.data.objects['MSUBAND'].modifiers['W'].mix_mode = 'SUB'
bpy.data.objects['MSUBAND'].modifiers['W'].mix_set = 'AND'
bpy.data.objects['MSUBAND'].modifiers['W'].default_weight_a = 0.2
bpy.data.objects['MSUBAND'].modifiers['W'].default_weight_b = 0.6
bpy.data.objects['MSUBAND'].modifiers.new(name='Probe', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['MSUBAND'].modifiers['Probe'].vertex_group = 'G'
bpy.data.objects['MSUBAND'].modifiers['Probe'].use_add = True
bpy.data.objects['MSUBAND'].modifiers['Probe'].add_threshold = 0.0
bpy.data.objects['MSUBAND'].modifiers['Probe'].default_weight = 0.333
bpy.data.objects['MSUBAND'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['MSUBAND'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['MSUBAND'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['MSUBAND'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['MSUBAND'].modifiers['Dz'].strength = 1.0
bpy.ops.mesh.primitive_grid_add(x_subdivisions=7, y_subdivisions=7, size=2)
bpy.context.object.name = 'MMULAND'
bpy.data.objects['MMULAND'].vertex_groups.new(name='G')
bpy.data.objects['MMULAND'].vertex_groups['G'].add([46, 54], 0.2, 'REPLACE')
bpy.data.objects['MMULAND'].vertex_groups['G'].add([17, 21], 0.3, 'REPLACE')
bpy.data.objects['MMULAND'].vertex_groups['G'].add([6, 9, 15, 38, 47, 51], 0.4, 'REPLACE')
bpy.data.objects['MMULAND'].vertex_groups['G'].add([20, 28, 30, 31, 37, 48], 0.5, 'REPLACE')
bpy.data.objects['MMULAND'].vertex_groups['G'].add([0, 2, 33, 44, 52], 0.6, 'REPLACE')
bpy.data.objects['MMULAND'].vertex_groups['G'].add([4, 8, 27, 40, 61], 0.7, 'REPLACE')
bpy.data.objects['MMULAND'].vertex_groups['G'].add([22, 41, 55, 62], 0.8, 'REPLACE')
bpy.data.objects['MMULAND'].vertex_groups['G'].add([23, 34, 45, 53], 0.9, 'REPLACE')
bpy.data.objects['MMULAND'].vertex_groups['G'].add([7, 10, 11], 1.0, 'REPLACE')
bpy.data.objects['MMULAND'].vertex_groups.new(name='B')
bpy.data.objects['MMULAND'].vertex_groups['B'].add([23, 28, 53], 0.1, 'REPLACE')
bpy.data.objects['MMULAND'].vertex_groups['B'].add([7, 63], 0.2, 'REPLACE')
bpy.data.objects['MMULAND'].vertex_groups['B'].add([17, 18, 40, 51], 0.3, 'REPLACE')
bpy.data.objects['MMULAND'].vertex_groups['B'].add([1, 26, 27, 50], 0.4, 'REPLACE')
bpy.data.objects['MMULAND'].vertex_groups['B'].add([45, 54], 0.5, 'REPLACE')
bpy.data.objects['MMULAND'].vertex_groups['B'].add([37, 41, 59], 0.6, 'REPLACE')
bpy.data.objects['MMULAND'].vertex_groups['B'].add([6, 19, 25, 31], 0.7, 'REPLACE')
bpy.data.objects['MMULAND'].vertex_groups['B'].add([3, 22, 32, 34, 36, 43, 58], 0.8, 'REPLACE')
bpy.data.objects['MMULAND'].vertex_groups['B'].add([2, 20], 0.9, 'REPLACE')
bpy.data.objects['MMULAND'].vertex_groups['B'].add([8, 14, 15, 24], 1.0, 'REPLACE')
bpy.data.objects['MMULAND'].modifiers.new(name='W', type='VERTEX_WEIGHT_MIX')
bpy.data.objects['MMULAND'].modifiers['W'].vertex_group_a = 'G'
bpy.data.objects['MMULAND'].modifiers['W'].vertex_group_b = 'B'
bpy.data.objects['MMULAND'].modifiers['W'].mix_mode = 'MUL'
bpy.data.objects['MMULAND'].modifiers['W'].mix_set = 'AND'
bpy.data.objects['MMULAND'].modifiers['W'].default_weight_a = 0.2
bpy.data.objects['MMULAND'].modifiers['W'].default_weight_b = 0.6
bpy.data.objects['MMULAND'].modifiers.new(name='Probe', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['MMULAND'].modifiers['Probe'].vertex_group = 'G'
bpy.data.objects['MMULAND'].modifiers['Probe'].use_add = True
bpy.data.objects['MMULAND'].modifiers['Probe'].add_threshold = 0.0
bpy.data.objects['MMULAND'].modifiers['Probe'].default_weight = 0.333
bpy.data.objects['MMULAND'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['MMULAND'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['MMULAND'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['MMULAND'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['MMULAND'].modifiers['Dz'].strength = 1.0
bpy.ops.mesh.primitive_grid_add(x_subdivisions=7, y_subdivisions=7, size=2)
bpy.context.object.name = 'MDIVAND'
bpy.data.objects['MDIVAND'].vertex_groups.new(name='G')
bpy.data.objects['MDIVAND'].vertex_groups['G'].add([5, 7, 36, 41, 44, 45, 46, 49, 58], 0.1, 'REPLACE')
bpy.data.objects['MDIVAND'].vertex_groups['G'].add([9], 0.2, 'REPLACE')
bpy.data.objects['MDIVAND'].vertex_groups['G'].add([17, 18, 23, 27, 34, 60], 0.3, 'REPLACE')
bpy.data.objects['MDIVAND'].vertex_groups['G'].add([3, 25, 31, 42], 0.4, 'REPLACE')
bpy.data.objects['MDIVAND'].vertex_groups['G'].add([30, 63], 0.5, 'REPLACE')
bpy.data.objects['MDIVAND'].vertex_groups['G'].add([8, 19, 20], 0.6, 'REPLACE')
bpy.data.objects['MDIVAND'].vertex_groups['G'].add([48, 51, 59], 0.7, 'REPLACE')
bpy.data.objects['MDIVAND'].vertex_groups['G'].add([6, 11, 14, 29, 38, 43, 50], 0.8, 'REPLACE')
bpy.data.objects['MDIVAND'].vertex_groups['G'].add([15, 16, 37, 52, 53, 61], 0.9, 'REPLACE')
bpy.data.objects['MDIVAND'].vertex_groups['G'].add([21, 47, 57], 1.0, 'REPLACE')
bpy.data.objects['MDIVAND'].vertex_groups.new(name='B')
bpy.data.objects['MDIVAND'].vertex_groups['B'].add([5, 6, 7, 13, 24, 30, 41, 57], 0.1, 'REPLACE')
bpy.data.objects['MDIVAND'].vertex_groups['B'].add([34], 0.2, 'REPLACE')
bpy.data.objects['MDIVAND'].vertex_groups['B'].add([25, 46, 52], 0.3, 'REPLACE')
bpy.data.objects['MDIVAND'].vertex_groups['B'].add([27, 31, 58, 60], 0.4, 'REPLACE')
bpy.data.objects['MDIVAND'].vertex_groups['B'].add([29], 0.5, 'REPLACE')
bpy.data.objects['MDIVAND'].vertex_groups['B'].add([8, 39, 48, 56, 61], 0.6, 'REPLACE')
bpy.data.objects['MDIVAND'].vertex_groups['B'].add([0, 4, 23], 0.7, 'REPLACE')
bpy.data.objects['MDIVAND'].vertex_groups['B'].add([14, 54], 0.8, 'REPLACE')
bpy.data.objects['MDIVAND'].vertex_groups['B'].add([15, 32, 33, 37, 42, 45], 0.9, 'REPLACE')
bpy.data.objects['MDIVAND'].vertex_groups['B'].add([18, 40, 47, 53, 55, 59], 1.0, 'REPLACE')
bpy.data.objects['MDIVAND'].modifiers.new(name='W', type='VERTEX_WEIGHT_MIX')
bpy.data.objects['MDIVAND'].modifiers['W'].vertex_group_a = 'G'
bpy.data.objects['MDIVAND'].modifiers['W'].vertex_group_b = 'B'
bpy.data.objects['MDIVAND'].modifiers['W'].mix_mode = 'DIV'
bpy.data.objects['MDIVAND'].modifiers['W'].mix_set = 'AND'
bpy.data.objects['MDIVAND'].modifiers['W'].default_weight_a = 0.2
bpy.data.objects['MDIVAND'].modifiers['W'].default_weight_b = 0.6
bpy.data.objects['MDIVAND'].modifiers.new(name='Probe', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['MDIVAND'].modifiers['Probe'].vertex_group = 'G'
bpy.data.objects['MDIVAND'].modifiers['Probe'].use_add = True
bpy.data.objects['MDIVAND'].modifiers['Probe'].add_threshold = 0.0
bpy.data.objects['MDIVAND'].modifiers['Probe'].default_weight = 0.333
bpy.data.objects['MDIVAND'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['MDIVAND'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['MDIVAND'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['MDIVAND'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['MDIVAND'].modifiers['Dz'].strength = 1.0
bpy.ops.mesh.primitive_grid_add(x_subdivisions=7, y_subdivisions=7, size=2)
bpy.context.object.name = 'MDIFAND'
bpy.data.objects['MDIFAND'].vertex_groups.new(name='G')
bpy.data.objects['MDIFAND'].vertex_groups['G'].add([7, 11, 38], 0.1, 'REPLACE')
bpy.data.objects['MDIFAND'].vertex_groups['G'].add([22, 48, 55, 58, 61], 0.2, 'REPLACE')
bpy.data.objects['MDIFAND'].vertex_groups['G'].add([3, 17, 21, 44], 0.3, 'REPLACE')
bpy.data.objects['MDIFAND'].vertex_groups['G'].add([5, 25, 56], 0.4, 'REPLACE')
bpy.data.objects['MDIFAND'].vertex_groups['G'].add([1, 2, 10, 16, 32, 47, 50, 57], 0.5, 'REPLACE')
bpy.data.objects['MDIFAND'].vertex_groups['G'].add([6, 28, 45, 63], 0.6, 'REPLACE')
bpy.data.objects['MDIFAND'].vertex_groups['G'].add([41, 53], 0.7, 'REPLACE')
bpy.data.objects['MDIFAND'].vertex_groups['G'].add([4, 30, 34, 39, 49], 0.8, 'REPLACE')
bpy.data.objects['MDIFAND'].vertex_groups['G'].add([8, 9, 20, 26, 29], 0.9, 'REPLACE')
bpy.data.objects['MDIFAND'].vertex_groups['G'].add([14, 15, 23, 40, 43, 59], 1.0, 'REPLACE')
bpy.data.objects['MDIFAND'].vertex_groups.new(name='B')
bpy.data.objects['MDIFAND'].vertex_groups['B'].add([30, 31, 36, 43, 55], 0.1, 'REPLACE')
bpy.data.objects['MDIFAND'].vertex_groups['B'].add([17, 28, 53, 58], 0.2, 'REPLACE')
bpy.data.objects['MDIFAND'].vertex_groups['B'].add([13, 18, 46], 0.3, 'REPLACE')
bpy.data.objects['MDIFAND'].vertex_groups['B'].add([4, 8, 19, 21, 62], 0.4, 'REPLACE')
bpy.data.objects['MDIFAND'].vertex_groups['B'].add([29, 35, 47, 48], 0.5, 'REPLACE')
bpy.data.objects['MDIFAND'].vertex_groups['B'].add([26, 32], 0.6, 'REPLACE')
bpy.data.objects['MDIFAND'].vertex_groups['B'].add([7, 25, 37, 50, 60], 0.7, 'REPLACE')
bpy.data.objects['MDIFAND'].vertex_groups['B'].add([5, 20, 59], 0.8, 'REPLACE')
bpy.data.objects['MDIFAND'].vertex_groups['B'].add([3, 12, 41, 52, 54], 0.9, 'REPLACE')
bpy.data.objects['MDIFAND'].vertex_groups['B'].add([0, 6, 23, 24, 34, 51], 1.0, 'REPLACE')
bpy.data.objects['MDIFAND'].modifiers.new(name='W', type='VERTEX_WEIGHT_MIX')
bpy.data.objects['MDIFAND'].modifiers['W'].vertex_group_a = 'G'
bpy.data.objects['MDIFAND'].modifiers['W'].vertex_group_b = 'B'
bpy.data.objects['MDIFAND'].modifiers['W'].mix_mode = 'DIF'
bpy.data.objects['MDIFAND'].modifiers['W'].mix_set = 'AND'
bpy.data.objects['MDIFAND'].modifiers['W'].default_weight_a = 0.2
bpy.data.objects['MDIFAND'].modifiers['W'].default_weight_b = 0.6
bpy.data.objects['MDIFAND'].modifiers.new(name='Probe', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['MDIFAND'].modifiers['Probe'].vertex_group = 'G'
bpy.data.objects['MDIFAND'].modifiers['Probe'].use_add = True
bpy.data.objects['MDIFAND'].modifiers['Probe'].add_threshold = 0.0
bpy.data.objects['MDIFAND'].modifiers['Probe'].default_weight = 0.333
bpy.data.objects['MDIFAND'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['MDIFAND'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['MDIFAND'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['MDIFAND'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['MDIFAND'].modifiers['Dz'].strength = 1.0
bpy.ops.mesh.primitive_grid_add(x_subdivisions=7, y_subdivisions=7, size=2)
bpy.context.object.name = 'MAVGAND'
bpy.data.objects['MAVGAND'].vertex_groups.new(name='G')
bpy.data.objects['MAVGAND'].vertex_groups['G'].add([13, 23, 25, 38, 39], 0.1, 'REPLACE')
bpy.data.objects['MAVGAND'].vertex_groups['G'].add([2, 36, 50, 63], 0.2, 'REPLACE')
bpy.data.objects['MAVGAND'].vertex_groups['G'].add([7, 8, 12, 14, 32, 55], 0.3, 'REPLACE')
bpy.data.objects['MAVGAND'].vertex_groups['G'].add([10, 26, 37, 47, 57], 0.4, 'REPLACE')
bpy.data.objects['MAVGAND'].vertex_groups['G'].add([1, 5, 6, 19, 30, 61], 0.5, 'REPLACE')
bpy.data.objects['MAVGAND'].vertex_groups['G'].add([9, 35, 40, 46, 59], 0.6, 'REPLACE')
bpy.data.objects['MAVGAND'].vertex_groups['G'].add([28], 0.7, 'REPLACE')
bpy.data.objects['MAVGAND'].vertex_groups['G'].add([33, 51, 58, 60], 0.8, 'REPLACE')
bpy.data.objects['MAVGAND'].vertex_groups['G'].add([16, 18, 20, 24, 29, 34], 0.9, 'REPLACE')
bpy.data.objects['MAVGAND'].vertex_groups['G'].add([21, 22, 45, 49, 54], 1.0, 'REPLACE')
bpy.data.objects['MAVGAND'].vertex_groups.new(name='B')
bpy.data.objects['MAVGAND'].vertex_groups['B'].add([18, 22, 28, 33, 59, 62], 0.1, 'REPLACE')
bpy.data.objects['MAVGAND'].vertex_groups['B'].add([8, 25, 52], 0.2, 'REPLACE')
bpy.data.objects['MAVGAND'].vertex_groups['B'].add([5, 10, 20, 58], 0.3, 'REPLACE')
bpy.data.objects['MAVGAND'].vertex_groups['B'].add([2, 13, 15, 24, 42, 51, 55], 0.4, 'REPLACE')
bpy.data.objects['MAVGAND'].vertex_groups['B'].add([14, 27, 31], 0.5, 'REPLACE')
bpy.data.objects['MAVGAND'].vertex_groups['B'].add([23, 54], 0.6, 'REPLACE')
bpy.data.objects['MAVGAND'].vertex_groups['B'].add([9, 32, 40, 43, 44, 47], 0.7, 'REPLACE')
bpy.data.objects['MAVGAND'].vertex_groups['B'].add([35, 57], 0.8, 'REPLACE')
bpy.data.objects['MAVGAND'].vertex_groups['B'].add([6, 46, 60], 1.0, 'REPLACE')
bpy.data.objects['MAVGAND'].modifiers.new(name='W', type='VERTEX_WEIGHT_MIX')
bpy.data.objects['MAVGAND'].modifiers['W'].vertex_group_a = 'G'
bpy.data.objects['MAVGAND'].modifiers['W'].vertex_group_b = 'B'
bpy.data.objects['MAVGAND'].modifiers['W'].mix_mode = 'AVG'
bpy.data.objects['MAVGAND'].modifiers['W'].mix_set = 'AND'
bpy.data.objects['MAVGAND'].modifiers['W'].default_weight_a = 0.2
bpy.data.objects['MAVGAND'].modifiers['W'].default_weight_b = 0.6
bpy.data.objects['MAVGAND'].modifiers.new(name='Probe', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['MAVGAND'].modifiers['Probe'].vertex_group = 'G'
bpy.data.objects['MAVGAND'].modifiers['Probe'].use_add = True
bpy.data.objects['MAVGAND'].modifiers['Probe'].add_threshold = 0.0
bpy.data.objects['MAVGAND'].modifiers['Probe'].default_weight = 0.333
bpy.data.objects['MAVGAND'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['MAVGAND'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['MAVGAND'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['MAVGAND'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['MAVGAND'].modifiers['Dz'].strength = 1.0
bpy.ops.mesh.primitive_grid_add(x_subdivisions=7, y_subdivisions=7, size=2)
bpy.context.object.name = 'MMINAND'
bpy.data.objects['MMINAND'].vertex_groups.new(name='G')
bpy.data.objects['MMINAND'].vertex_groups['G'].add([11, 16, 24], 0.1, 'REPLACE')
bpy.data.objects['MMINAND'].vertex_groups['G'].add([3, 12, 22], 0.2, 'REPLACE')
bpy.data.objects['MMINAND'].vertex_groups['G'].add([0, 1, 23, 37, 47], 0.3, 'REPLACE')
bpy.data.objects['MMINAND'].vertex_groups['G'].add([15, 29, 39, 46], 0.4, 'REPLACE')
bpy.data.objects['MMINAND'].vertex_groups['G'].add([27, 31, 60], 0.5, 'REPLACE')
bpy.data.objects['MMINAND'].vertex_groups['G'].add([2, 4, 13, 38, 40, 42, 43, 49, 51], 0.6, 'REPLACE')
bpy.data.objects['MMINAND'].vertex_groups['G'].add([18, 52], 0.7, 'REPLACE')
bpy.data.objects['MMINAND'].vertex_groups['G'].add([10, 28, 57, 61, 63], 0.8, 'REPLACE')
bpy.data.objects['MMINAND'].vertex_groups['G'].add([17, 44, 58, 62], 0.9, 'REPLACE')
bpy.data.objects['MMINAND'].vertex_groups['G'].add([19, 30, 45, 48, 55], 1.0, 'REPLACE')
bpy.data.objects['MMINAND'].vertex_groups.new(name='B')
bpy.data.objects['MMINAND'].vertex_groups['B'].add([9, 10, 37, 55], 0.1, 'REPLACE')
bpy.data.objects['MMINAND'].vertex_groups['B'].add([39, 48], 0.2, 'REPLACE')
bpy.data.objects['MMINAND'].vertex_groups['B'].add([20, 25, 26, 54, 63], 0.3, 'REPLACE')
bpy.data.objects['MMINAND'].vertex_groups['B'].add([3, 5, 16, 18, 32, 47], 0.4, 'REPLACE')
bpy.data.objects['MMINAND'].vertex_groups['B'].add([0, 7, 46], 0.5, 'REPLACE')
bpy.data.objects['MMINAND'].vertex_groups['B'].add([6, 8, 19, 30, 44, 50], 0.6, 'REPLACE')
bpy.data.objects['MMINAND'].vertex_groups['B'].add([11, 21, 27, 42, 51, 60], 0.7, 'REPLACE')
bpy.data.objects['MMINAND'].vertex_groups['B'].add([12, 28, 36, 40, 53, 56, 62], 0.8, 'REPLACE')
bpy.data.objects['MMINAND'].vertex_groups['B'].add([2, 14, 31, 43, 57], 0.9, 'REPLACE')
bpy.data.objects['MMINAND'].vertex_groups['B'].add([4, 13, 17, 29], 1.0, 'REPLACE')
bpy.data.objects['MMINAND'].modifiers.new(name='W', type='VERTEX_WEIGHT_MIX')
bpy.data.objects['MMINAND'].modifiers['W'].vertex_group_a = 'G'
bpy.data.objects['MMINAND'].modifiers['W'].vertex_group_b = 'B'
bpy.data.objects['MMINAND'].modifiers['W'].mix_mode = 'MIN'
bpy.data.objects['MMINAND'].modifiers['W'].mix_set = 'AND'
bpy.data.objects['MMINAND'].modifiers['W'].default_weight_a = 0.2
bpy.data.objects['MMINAND'].modifiers['W'].default_weight_b = 0.6
bpy.data.objects['MMINAND'].modifiers.new(name='Probe', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['MMINAND'].modifiers['Probe'].vertex_group = 'G'
bpy.data.objects['MMINAND'].modifiers['Probe'].use_add = True
bpy.data.objects['MMINAND'].modifiers['Probe'].add_threshold = 0.0
bpy.data.objects['MMINAND'].modifiers['Probe'].default_weight = 0.333
bpy.data.objects['MMINAND'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['MMINAND'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['MMINAND'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['MMINAND'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['MMINAND'].modifiers['Dz'].strength = 1.0
bpy.ops.mesh.primitive_grid_add(x_subdivisions=7, y_subdivisions=7, size=2)
bpy.context.object.name = 'MMAXAND'
bpy.data.objects['MMAXAND'].vertex_groups.new(name='G')
bpy.data.objects['MMAXAND'].vertex_groups['G'].add([3, 17, 47, 49], 0.1, 'REPLACE')
bpy.data.objects['MMAXAND'].vertex_groups['G'].add([1, 31, 34, 46, 53, 61], 0.2, 'REPLACE')
bpy.data.objects['MMAXAND'].vertex_groups['G'].add([9, 24], 0.3, 'REPLACE')
bpy.data.objects['MMAXAND'].vertex_groups['G'].add([38, 42], 0.4, 'REPLACE')
bpy.data.objects['MMAXAND'].vertex_groups['G'].add([5, 20, 33, 37, 50, 58], 0.5, 'REPLACE')
bpy.data.objects['MMAXAND'].vertex_groups['G'].add([25, 41, 45, 59], 0.6, 'REPLACE')
bpy.data.objects['MMAXAND'].vertex_groups['G'].add([0, 54], 0.7, 'REPLACE')
bpy.data.objects['MMAXAND'].vertex_groups['G'].add([7, 16, 26, 52, 60], 0.8, 'REPLACE')
bpy.data.objects['MMAXAND'].vertex_groups['G'].add([8, 13, 56, 62], 0.9, 'REPLACE')
bpy.data.objects['MMAXAND'].vertex_groups['G'].add([11, 21, 30, 32, 43, 44], 1.0, 'REPLACE')
bpy.data.objects['MMAXAND'].vertex_groups.new(name='B')
bpy.data.objects['MMAXAND'].vertex_groups['B'].add([7, 21, 30, 52, 61], 0.1, 'REPLACE')
bpy.data.objects['MMAXAND'].vertex_groups['B'].add([2, 5, 24, 47, 59], 0.2, 'REPLACE')
bpy.data.objects['MMAXAND'].vertex_groups['B'].add([46, 50], 0.3, 'REPLACE')
bpy.data.objects['MMAXAND'].vertex_groups['B'].add([55, 60], 0.4, 'REPLACE')
bpy.data.objects['MMAXAND'].vertex_groups['B'].add([49], 0.5, 'REPLACE')
bpy.data.objects['MMAXAND'].vertex_groups['B'].add([20, 28, 29, 32, 34], 0.6, 'REPLACE')
bpy.data.objects['MMAXAND'].vertex_groups['B'].add([57], 0.7, 'REPLACE')
bpy.data.objects['MMAXAND'].vertex_groups['B'].add([1, 4, 8, 17, 42], 0.8, 'REPLACE')
bpy.data.objects['MMAXAND'].vertex_groups['B'].add([33, 43, 44, 45, 62], 0.9, 'REPLACE')
bpy.data.objects['MMAXAND'].vertex_groups['B'].add([6, 9, 18, 48], 1.0, 'REPLACE')
bpy.data.objects['MMAXAND'].modifiers.new(name='W', type='VERTEX_WEIGHT_MIX')
bpy.data.objects['MMAXAND'].modifiers['W'].vertex_group_a = 'G'
bpy.data.objects['MMAXAND'].modifiers['W'].vertex_group_b = 'B'
bpy.data.objects['MMAXAND'].modifiers['W'].mix_mode = 'MAX'
bpy.data.objects['MMAXAND'].modifiers['W'].mix_set = 'AND'
bpy.data.objects['MMAXAND'].modifiers['W'].default_weight_a = 0.2
bpy.data.objects['MMAXAND'].modifiers['W'].default_weight_b = 0.6
bpy.data.objects['MMAXAND'].modifiers.new(name='Probe', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['MMAXAND'].modifiers['Probe'].vertex_group = 'G'
bpy.data.objects['MMAXAND'].modifiers['Probe'].use_add = True
bpy.data.objects['MMAXAND'].modifiers['Probe'].add_threshold = 0.0
bpy.data.objects['MMAXAND'].modifiers['Probe'].default_weight = 0.333
bpy.data.objects['MMAXAND'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['MMAXAND'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['MMAXAND'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['MMAXAND'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['MMAXAND'].modifiers['Dz'].strength = 1.0
bpy.ops.mesh.primitive_grid_add(x_subdivisions=7, y_subdivisions=7, size=2)
bpy.context.object.name = 'XInvA'
bpy.data.objects['XInvA'].vertex_groups.new(name='G')
bpy.data.objects['XInvA'].vertex_groups['G'].add([17, 30, 40, 54, 55, 58], 0.1, 'REPLACE')
bpy.data.objects['XInvA'].vertex_groups['G'].add([20, 29, 33, 39, 43], 0.2, 'REPLACE')
bpy.data.objects['XInvA'].vertex_groups['G'].add([2, 34, 56, 60], 0.3, 'REPLACE')
bpy.data.objects['XInvA'].vertex_groups['G'].add([27, 36], 0.4, 'REPLACE')
bpy.data.objects['XInvA'].vertex_groups['G'].add([23], 0.5, 'REPLACE')
bpy.data.objects['XInvA'].vertex_groups['G'].add([8, 13, 31], 0.6, 'REPLACE')
bpy.data.objects['XInvA'].vertex_groups['G'].add([5, 12, 50], 0.7, 'REPLACE')
bpy.data.objects['XInvA'].vertex_groups['G'].add([25, 45, 57, 61, 62], 0.8, 'REPLACE')
bpy.data.objects['XInvA'].vertex_groups['G'].add([3, 32, 37, 47, 48, 53, 59], 0.9, 'REPLACE')
bpy.data.objects['XInvA'].vertex_groups['G'].add([11, 16, 18, 35], 1.0, 'REPLACE')
bpy.data.objects['XInvA'].vertex_groups.new(name='B')
bpy.data.objects['XInvA'].vertex_groups['B'].add([24, 29, 49], 0.1, 'REPLACE')
bpy.data.objects['XInvA'].vertex_groups['B'].add([4, 13, 19, 44], 0.2, 'REPLACE')
bpy.data.objects['XInvA'].vertex_groups['B'].add([3, 15, 27, 42, 56], 0.3, 'REPLACE')
bpy.data.objects['XInvA'].vertex_groups['B'].add([12, 18, 23, 25, 28, 38, 47, 60], 0.4, 'REPLACE')
bpy.data.objects['XInvA'].vertex_groups['B'].add([1, 21, 51, 57, 62], 0.5, 'REPLACE')
bpy.data.objects['XInvA'].vertex_groups['B'].add([14, 50], 0.6, 'REPLACE')
bpy.data.objects['XInvA'].vertex_groups['B'].add([5, 16, 20, 48], 0.7, 'REPLACE')
bpy.data.objects['XInvA'].vertex_groups['B'].add([37, 53, 59], 0.8, 'REPLACE')
bpy.data.objects['XInvA'].vertex_groups['B'].add([0, 35, 46, 52], 0.9, 'REPLACE')
bpy.data.objects['XInvA'].vertex_groups['B'].add([10, 11, 34, 40], 1.0, 'REPLACE')
bpy.data.objects['XInvA'].modifiers.new(name='W', type='VERTEX_WEIGHT_MIX')
bpy.data.objects['XInvA'].modifiers['W'].vertex_group_a = 'G'
bpy.data.objects['XInvA'].modifiers['W'].vertex_group_b = 'B'
bpy.data.objects['XInvA'].modifiers['W'].default_weight_a = 0.2
bpy.data.objects['XInvA'].modifiers['W'].default_weight_b = 0.6
bpy.data.objects['XInvA'].modifiers['W'].invert_vertex_group_a = True
bpy.data.objects['XInvA'].modifiers['W'].mix_mode = 'ADD'
bpy.data.objects['XInvA'].modifiers['W'].mix_set = 'OR'
bpy.data.objects['XInvA'].modifiers.new(name='Probe', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['XInvA'].modifiers['Probe'].vertex_group = 'G'
bpy.data.objects['XInvA'].modifiers['Probe'].use_add = True
bpy.data.objects['XInvA'].modifiers['Probe'].add_threshold = 0.0
bpy.data.objects['XInvA'].modifiers['Probe'].default_weight = 0.333
bpy.data.objects['XInvA'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['XInvA'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['XInvA'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['XInvA'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['XInvA'].modifiers['Dz'].strength = 1.0
bpy.ops.mesh.primitive_grid_add(x_subdivisions=7, y_subdivisions=7, size=2)
bpy.context.object.name = 'XInvB'
bpy.data.objects['XInvB'].vertex_groups.new(name='G')
bpy.data.objects['XInvB'].vertex_groups['G'].add([6, 54, 56], 0.1, 'REPLACE')
bpy.data.objects['XInvB'].vertex_groups['G'].add([1, 10, 25, 55], 0.2, 'REPLACE')
bpy.data.objects['XInvB'].vertex_groups['G'].add([16, 19, 28, 63], 0.3, 'REPLACE')
bpy.data.objects['XInvB'].vertex_groups['G'].add([8, 14], 0.4, 'REPLACE')
bpy.data.objects['XInvB'].vertex_groups['G'].add([7, 12, 15, 27, 36], 0.5, 'REPLACE')
bpy.data.objects['XInvB'].vertex_groups['G'].add([39, 41, 45, 61], 0.6, 'REPLACE')
bpy.data.objects['XInvB'].vertex_groups['G'].add([2, 5, 17, 18, 23, 35, 42, 62], 0.7, 'REPLACE')
bpy.data.objects['XInvB'].vertex_groups['G'].add([3, 9, 21, 31, 37, 53, 57], 0.8, 'REPLACE')
bpy.data.objects['XInvB'].vertex_groups['G'].add([0, 22, 24, 30, 49], 0.9, 'REPLACE')
bpy.data.objects['XInvB'].vertex_groups['G'].add([20, 26, 52], 1.0, 'REPLACE')
bpy.data.objects['XInvB'].vertex_groups.new(name='B')
bpy.data.objects['XInvB'].vertex_groups['B'].add([6, 10, 16, 30, 40, 52, 61], 0.1, 'REPLACE')
bpy.data.objects['XInvB'].vertex_groups['B'].add([0, 8, 26, 37], 0.2, 'REPLACE')
bpy.data.objects['XInvB'].vertex_groups['B'].add([14, 24, 35, 41, 62], 0.3, 'REPLACE')
bpy.data.objects['XInvB'].vertex_groups['B'].add([2, 13, 38, 45, 48, 49], 0.4, 'REPLACE')
bpy.data.objects['XInvB'].vertex_groups['B'].add([15, 22, 39, 42, 43, 44, 58], 0.5, 'REPLACE')
bpy.data.objects['XInvB'].vertex_groups['B'].add([1, 4, 23, 60], 0.6, 'REPLACE')
bpy.data.objects['XInvB'].vertex_groups['B'].add([7, 9, 19, 32, 36, 54, 57], 0.7, 'REPLACE')
bpy.data.objects['XInvB'].vertex_groups['B'].add([21, 25, 34, 50, 53], 0.8, 'REPLACE')
bpy.data.objects['XInvB'].vertex_groups['B'].add([27, 28, 63], 0.9, 'REPLACE')
bpy.data.objects['XInvB'].vertex_groups['B'].add([3, 11, 46, 51, 56], 1.0, 'REPLACE')
bpy.data.objects['XInvB'].modifiers.new(name='W', type='VERTEX_WEIGHT_MIX')
bpy.data.objects['XInvB'].modifiers['W'].vertex_group_a = 'G'
bpy.data.objects['XInvB'].modifiers['W'].vertex_group_b = 'B'
bpy.data.objects['XInvB'].modifiers['W'].default_weight_a = 0.2
bpy.data.objects['XInvB'].modifiers['W'].default_weight_b = 0.6
bpy.data.objects['XInvB'].modifiers['W'].invert_vertex_group_b = True
bpy.data.objects['XInvB'].modifiers['W'].mix_mode = 'SUB'
bpy.data.objects['XInvB'].modifiers['W'].mix_set = 'ALL'
bpy.data.objects['XInvB'].modifiers.new(name='Probe', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['XInvB'].modifiers['Probe'].vertex_group = 'G'
bpy.data.objects['XInvB'].modifiers['Probe'].use_add = True
bpy.data.objects['XInvB'].modifiers['Probe'].add_threshold = 0.0
bpy.data.objects['XInvB'].modifiers['Probe'].default_weight = 0.333
bpy.data.objects['XInvB'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['XInvB'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['XInvB'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['XInvB'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['XInvB'].modifiers['Dz'].strength = 1.0
bpy.ops.mesh.primitive_grid_add(x_subdivisions=7, y_subdivisions=7, size=2)
bpy.context.object.name = 'XNorm'
bpy.data.objects['XNorm'].vertex_groups.new(name='G')
bpy.data.objects['XNorm'].vertex_groups['G'].add([50], 0.1, 'REPLACE')
bpy.data.objects['XNorm'].vertex_groups['G'].add([11, 12, 28, 44, 55, 56], 0.2, 'REPLACE')
bpy.data.objects['XNorm'].vertex_groups['G'].add([9, 18, 26, 40, 42, 61], 0.3, 'REPLACE')
bpy.data.objects['XNorm'].vertex_groups['G'].add([10, 16], 0.4, 'REPLACE')
bpy.data.objects['XNorm'].vertex_groups['G'].add([6, 17, 32, 35, 47, 63], 0.5, 'REPLACE')
bpy.data.objects['XNorm'].vertex_groups['G'].add([54], 0.6, 'REPLACE')
bpy.data.objects['XNorm'].vertex_groups['G'].add([20, 30, 36, 51], 0.7, 'REPLACE')
bpy.data.objects['XNorm'].vertex_groups['G'].add([5, 15, 19, 34], 0.8, 'REPLACE')
bpy.data.objects['XNorm'].vertex_groups['G'].add([2, 45, 53], 0.9, 'REPLACE')
bpy.data.objects['XNorm'].vertex_groups['G'].add([13, 58], 1.0, 'REPLACE')
bpy.data.objects['XNorm'].vertex_groups.new(name='B')
bpy.data.objects['XNorm'].vertex_groups['B'].add([16, 20, 21, 47], 0.1, 'REPLACE')
bpy.data.objects['XNorm'].vertex_groups['B'].add([34, 51, 52], 0.2, 'REPLACE')
bpy.data.objects['XNorm'].vertex_groups['B'].add([8, 22], 0.3, 'REPLACE')
bpy.data.objects['XNorm'].vertex_groups['B'].add([5, 27, 31, 35, 38, 42], 0.4, 'REPLACE')
bpy.data.objects['XNorm'].vertex_groups['B'].add([13, 30, 40, 44], 0.5, 'REPLACE')
bpy.data.objects['XNorm'].vertex_groups['B'].add([18], 0.6, 'REPLACE')
bpy.data.objects['XNorm'].vertex_groups['B'].add([2, 3, 9, 23, 49, 50, 55], 0.7, 'REPLACE')
bpy.data.objects['XNorm'].vertex_groups['B'].add([1, 4, 19, 36, 41, 46, 53], 0.8, 'REPLACE')
bpy.data.objects['XNorm'].vertex_groups['B'].add([0, 7, 12, 56, 61], 0.9, 'REPLACE')
bpy.data.objects['XNorm'].vertex_groups['B'].add([15, 17, 43, 48, 58, 63], 1.0, 'REPLACE')
bpy.data.objects['XNorm'].modifiers.new(name='W', type='VERTEX_WEIGHT_MIX')
bpy.data.objects['XNorm'].modifiers['W'].vertex_group_a = 'G'
bpy.data.objects['XNorm'].modifiers['W'].vertex_group_b = 'B'
bpy.data.objects['XNorm'].modifiers['W'].default_weight_a = 0.2
bpy.data.objects['XNorm'].modifiers['W'].default_weight_b = 0.6
bpy.data.objects['XNorm'].modifiers['W'].normalize = True
bpy.data.objects['XNorm'].modifiers['W'].mix_mode = 'ADD'
bpy.data.objects['XNorm'].modifiers['W'].mix_set = 'ALL'
bpy.data.objects['XNorm'].modifiers.new(name='Probe', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['XNorm'].modifiers['Probe'].vertex_group = 'G'
bpy.data.objects['XNorm'].modifiers['Probe'].use_add = True
bpy.data.objects['XNorm'].modifiers['Probe'].add_threshold = 0.0
bpy.data.objects['XNorm'].modifiers['Probe'].default_weight = 0.333
bpy.data.objects['XNorm'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['XNorm'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['XNorm'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['XNorm'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['XNorm'].modifiers['Dz'].strength = 1.0
bpy.ops.mesh.primitive_grid_add(x_subdivisions=7, y_subdivisions=7, size=2)
bpy.context.object.name = 'XInfluence'
bpy.data.objects['XInfluence'].vertex_groups.new(name='G')
bpy.data.objects['XInfluence'].vertex_groups['G'].add([9, 26], 0.1, 'REPLACE')
bpy.data.objects['XInfluence'].vertex_groups['G'].add([16, 25, 48, 49, 56, 57, 59], 0.2, 'REPLACE')
bpy.data.objects['XInfluence'].vertex_groups['G'].add([34, 35, 42], 0.3, 'REPLACE')
bpy.data.objects['XInfluence'].vertex_groups['G'].add([5, 24, 43], 0.4, 'REPLACE')
bpy.data.objects['XInfluence'].vertex_groups['G'].add([12, 37, 38, 39], 0.5, 'REPLACE')
bpy.data.objects['XInfluence'].vertex_groups['G'].add([3, 32, 33], 0.6, 'REPLACE')
bpy.data.objects['XInfluence'].vertex_groups['G'].add([4, 7, 8, 13, 23, 30, 31, 51], 0.7, 'REPLACE')
bpy.data.objects['XInfluence'].vertex_groups['G'].add([17, 18, 21, 45], 0.8, 'REPLACE')
bpy.data.objects['XInfluence'].vertex_groups['G'].add([50, 61], 0.9, 'REPLACE')
bpy.data.objects['XInfluence'].vertex_groups['G'].add([27, 46, 62], 1.0, 'REPLACE')
bpy.data.objects['XInfluence'].vertex_groups.new(name='B')
bpy.data.objects['XInfluence'].vertex_groups['B'].add([15, 22, 25, 33, 43, 57, 62], 0.1, 'REPLACE')
bpy.data.objects['XInfluence'].vertex_groups['B'].add([7, 11, 24, 30, 42, 50, 58, 60, 63], 0.2, 'REPLACE')
bpy.data.objects['XInfluence'].vertex_groups['B'].add([9, 14, 45, 52], 0.3, 'REPLACE')
bpy.data.objects['XInfluence'].vertex_groups['B'].add([0, 23, 37, 41], 0.4, 'REPLACE')
bpy.data.objects['XInfluence'].vertex_groups['B'].add([13], 0.5, 'REPLACE')
bpy.data.objects['XInfluence'].vertex_groups['B'].add([38, 54, 55], 0.6, 'REPLACE')
bpy.data.objects['XInfluence'].vertex_groups['B'].add([6, 32], 0.7, 'REPLACE')
bpy.data.objects['XInfluence'].vertex_groups['B'].add([2, 19, 28, 39, 49, 59, 61], 0.8, 'REPLACE')
bpy.data.objects['XInfluence'].vertex_groups['B'].add([3, 26, 29, 34], 0.9, 'REPLACE')
bpy.data.objects['XInfluence'].vertex_groups['B'].add([10, 20, 27, 56], 1.0, 'REPLACE')
bpy.data.objects['XInfluence'].modifiers.new(name='W', type='VERTEX_WEIGHT_MIX')
bpy.data.objects['XInfluence'].modifiers['W'].vertex_group_a = 'G'
bpy.data.objects['XInfluence'].modifiers['W'].vertex_group_b = 'B'
bpy.data.objects['XInfluence'].modifiers['W'].default_weight_a = 0.2
bpy.data.objects['XInfluence'].modifiers['W'].default_weight_b = 0.6
bpy.data.objects['XInfluence'].modifiers['W'].mask_constant = 0.3
bpy.data.objects['XInfluence'].modifiers['W'].mix_mode = 'SET'
bpy.data.objects['XInfluence'].modifiers['W'].mix_set = 'ALL'
bpy.data.objects['XInfluence'].modifiers.new(name='Probe', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['XInfluence'].modifiers['Probe'].vertex_group = 'G'
bpy.data.objects['XInfluence'].modifiers['Probe'].use_add = True
bpy.data.objects['XInfluence'].modifiers['Probe'].add_threshold = 0.0
bpy.data.objects['XInfluence'].modifiers['Probe'].default_weight = 0.333
bpy.data.objects['XInfluence'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['XInfluence'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['XInfluence'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['XInfluence'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['XInfluence'].modifiers['Dz'].strength = 1.0
bpy.ops.mesh.primitive_grid_add(x_subdivisions=7, y_subdivisions=7, size=2)
bpy.context.object.name = 'XMaskGroup'
bpy.data.objects['XMaskGroup'].vertex_groups.new(name='G')
bpy.data.objects['XMaskGroup'].vertex_groups['G'].add([10, 27, 39, 49, 63], 0.1, 'REPLACE')
bpy.data.objects['XMaskGroup'].vertex_groups['G'].add([14, 33], 0.2, 'REPLACE')
bpy.data.objects['XMaskGroup'].vertex_groups['G'].add([20, 25, 35], 0.3, 'REPLACE')
bpy.data.objects['XMaskGroup'].vertex_groups['G'].add([15, 28], 0.4, 'REPLACE')
bpy.data.objects['XMaskGroup'].vertex_groups['G'].add([5, 12, 43, 54, 57], 0.5, 'REPLACE')
bpy.data.objects['XMaskGroup'].vertex_groups['G'].add([19, 24, 26], 0.6, 'REPLACE')
bpy.data.objects['XMaskGroup'].vertex_groups['G'].add([3, 6, 7, 23, 42, 47, 48, 51, 59, 61], 0.7, 'REPLACE')
bpy.data.objects['XMaskGroup'].vertex_groups['G'].add([16, 37], 0.8, 'REPLACE')
bpy.data.objects['XMaskGroup'].vertex_groups['G'].add([2, 4, 17, 31, 34, 52, 53], 0.9, 'REPLACE')
bpy.data.objects['XMaskGroup'].vertex_groups['G'].add([9, 18, 21, 29, 40], 1.0, 'REPLACE')
bpy.data.objects['XMaskGroup'].vertex_groups.new(name='B')
bpy.data.objects['XMaskGroup'].vertex_groups['B'].add([9, 10, 18], 0.1, 'REPLACE')
bpy.data.objects['XMaskGroup'].vertex_groups['B'].add([4, 5, 31, 34, 61], 0.2, 'REPLACE')
bpy.data.objects['XMaskGroup'].vertex_groups['B'].add([0, 1, 13, 20, 23, 28, 54], 0.3, 'REPLACE')
bpy.data.objects['XMaskGroup'].vertex_groups['B'].add([37, 38, 59], 0.4, 'REPLACE')
bpy.data.objects['XMaskGroup'].vertex_groups['B'].add([3, 14, 55], 0.5, 'REPLACE')
bpy.data.objects['XMaskGroup'].vertex_groups['B'].add([11, 12, 36, 41, 57, 62, 63], 0.6, 'REPLACE')
bpy.data.objects['XMaskGroup'].vertex_groups['B'].add([26, 43], 0.7, 'REPLACE')
bpy.data.objects['XMaskGroup'].vertex_groups['B'].add([8, 32, 33, 42], 0.8, 'REPLACE')
bpy.data.objects['XMaskGroup'].vertex_groups['B'].add([19, 21, 35, 47, 53, 56], 0.9, 'REPLACE')
bpy.data.objects['XMaskGroup'].vertex_groups['B'].add([17, 29, 46], 1.0, 'REPLACE')
bpy.data.objects['XMaskGroup'].vertex_groups.new(name='M')
bpy.data.objects['XMaskGroup'].vertex_groups['M'].add([0, 3, 7, 14, 28, 40, 42, 49, 54], 0.1, 'REPLACE')
bpy.data.objects['XMaskGroup'].vertex_groups['M'].add([4, 8, 12, 20, 21, 26], 0.2, 'REPLACE')
bpy.data.objects['XMaskGroup'].vertex_groups['M'].add([1, 18, 24, 31], 0.3, 'REPLACE')
bpy.data.objects['XMaskGroup'].vertex_groups['M'].add([2, 51, 57], 0.4, 'REPLACE')
bpy.data.objects['XMaskGroup'].vertex_groups['M'].add([11, 19, 43, 47, 52, 62], 0.5, 'REPLACE')
bpy.data.objects['XMaskGroup'].vertex_groups['M'].add([32, 34, 35, 44, 61], 0.6, 'REPLACE')
bpy.data.objects['XMaskGroup'].vertex_groups['M'].add([9, 22, 38, 50, 53], 0.7, 'REPLACE')
bpy.data.objects['XMaskGroup'].vertex_groups['M'].add([6, 25, 27, 30, 37, 59], 0.8, 'REPLACE')
bpy.data.objects['XMaskGroup'].vertex_groups['M'].add([13, 17, 41, 55], 0.9, 'REPLACE')
bpy.data.objects['XMaskGroup'].vertex_groups['M'].add([15, 16, 36, 39, 45, 46, 48, 58, 60, 63], 1.0, 'REPLACE')
bpy.data.objects['XMaskGroup'].modifiers.new(name='W', type='VERTEX_WEIGHT_MIX')
bpy.data.objects['XMaskGroup'].modifiers['W'].vertex_group_a = 'G'
bpy.data.objects['XMaskGroup'].modifiers['W'].vertex_group_b = 'B'
bpy.data.objects['XMaskGroup'].modifiers['W'].default_weight_a = 0.2
bpy.data.objects['XMaskGroup'].modifiers['W'].default_weight_b = 0.6
bpy.data.objects['XMaskGroup'].modifiers['W'].mask_vertex_group = 'M'
bpy.data.objects['XMaskGroup'].modifiers['W'].mix_mode = 'MAX'
bpy.data.objects['XMaskGroup'].modifiers['W'].mix_set = 'OR'
bpy.data.objects['XMaskGroup'].modifiers.new(name='Probe', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['XMaskGroup'].modifiers['Probe'].vertex_group = 'G'
bpy.data.objects['XMaskGroup'].modifiers['Probe'].use_add = True
bpy.data.objects['XMaskGroup'].modifiers['Probe'].add_threshold = 0.0
bpy.data.objects['XMaskGroup'].modifiers['Probe'].default_weight = 0.333
bpy.data.objects['XMaskGroup'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['XMaskGroup'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['XMaskGroup'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['XMaskGroup'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['XMaskGroup'].modifiers['Dz'].strength = 1.0
bpy.ops.mesh.primitive_grid_add(x_subdivisions=7, y_subdivisions=7, size=2)
bpy.context.object.name = 'XNoB'
bpy.data.objects['XNoB'].vertex_groups.new(name='G')
bpy.data.objects['XNoB'].vertex_groups['G'].add([17, 20, 42, 49, 59], 0.1, 'REPLACE')
bpy.data.objects['XNoB'].vertex_groups['G'].add([6, 32, 34, 37, 63], 0.2, 'REPLACE')
bpy.data.objects['XNoB'].vertex_groups['G'].add([25, 51, 54], 0.3, 'REPLACE')
bpy.data.objects['XNoB'].vertex_groups['G'].add([3, 33, 56], 0.4, 'REPLACE')
bpy.data.objects['XNoB'].vertex_groups['G'].add([27, 38, 45], 0.5, 'REPLACE')
bpy.data.objects['XNoB'].vertex_groups['G'].add([4, 22, 24, 29, 35, 43, 57], 0.6, 'REPLACE')
bpy.data.objects['XNoB'].vertex_groups['G'].add([8, 11, 12, 13, 41, 46, 55], 0.7, 'REPLACE')
bpy.data.objects['XNoB'].vertex_groups['G'].add([5, 7, 31], 0.8, 'REPLACE')
bpy.data.objects['XNoB'].vertex_groups['G'].add([10, 14, 23, 58], 0.9, 'REPLACE')
bpy.data.objects['XNoB'].vertex_groups['G'].add([36, 48, 50], 1.0, 'REPLACE')
bpy.data.objects['XNoB'].vertex_groups.new(name='B')
bpy.data.objects['XNoB'].vertex_groups['B'].add([19, 25, 31, 38], 0.1, 'REPLACE')
bpy.data.objects['XNoB'].vertex_groups['B'].add([10, 15, 18, 27, 46, 48, 57], 0.2, 'REPLACE')
bpy.data.objects['XNoB'].vertex_groups['B'].add([9, 14, 26, 28, 51], 0.3, 'REPLACE')
bpy.data.objects['XNoB'].vertex_groups['B'].add([1, 58, 60, 63], 0.4, 'REPLACE')
bpy.data.objects['XNoB'].vertex_groups['B'].add([23, 34, 59], 0.6, 'REPLACE')
bpy.data.objects['XNoB'].vertex_groups['B'].add([4, 45, 52, 55, 61], 0.7, 'REPLACE')
bpy.data.objects['XNoB'].vertex_groups['B'].add([0, 17, 30, 47], 0.8, 'REPLACE')
bpy.data.objects['XNoB'].vertex_groups['B'].add([5, 6, 37, 41, 42, 49], 0.9, 'REPLACE')
bpy.data.objects['XNoB'].vertex_groups['B'].add([3, 35], 1.0, 'REPLACE')
bpy.data.objects['XNoB'].modifiers.new(name='W', type='VERTEX_WEIGHT_MIX')
bpy.data.objects['XNoB'].modifiers['W'].vertex_group_a = 'G'
bpy.data.objects['XNoB'].modifiers['W'].vertex_group_b = ''
bpy.data.objects['XNoB'].modifiers['W'].default_weight_a = 0.2
bpy.data.objects['XNoB'].modifiers['W'].default_weight_b = 0.6
bpy.data.objects['XNoB'].modifiers['W'].mix_mode = 'ADD'
bpy.data.objects['XNoB'].modifiers['W'].mix_set = 'ALL'
bpy.data.objects['XNoB'].modifiers.new(name='Probe', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['XNoB'].modifiers['Probe'].vertex_group = 'G'
bpy.data.objects['XNoB'].modifiers['Probe'].use_add = True
bpy.data.objects['XNoB'].modifiers['Probe'].add_threshold = 0.0
bpy.data.objects['XNoB'].modifiers['Probe'].default_weight = 0.333
bpy.data.objects['XNoB'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['XNoB'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['XNoB'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['XNoB'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['XNoB'].modifiers['Dz'].strength = 1.0
