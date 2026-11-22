# WP 8.3.13: DATA_TRANSFER of vertex groups onto destination weights: mix_mode REPLACE
# (factor 0.5), ABOVE_THRESHOLD / BELOW_THRESHOLD (factor = threshold), MIX, ADD, SUB,
# MUL; the destination vertex_group mask (and inverted); groups matched by INDEX (the
# destination's X, Y, Z, W get the source's G, H, K); use_max_distance; local space;
# a destination lacking group H (not created by the modifier).
# golden: startup=empty compare=evaluated:DRep,evaluated:DAbove,evaluated:DBelow,evaluated:DMix,evaluated:DAdd,evaluated:DSub,evaluated:DMul,evaluated:DMask,evaluated:DMaskInv,evaluated:DIndex,evaluated:DMaxDist,evaluated:DLocal,evaluated:DNoH tier=T2
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=6, y_subdivisions=5, size=2)
bpy.context.object.name = 'S'
bpy.data.objects['S'].vertex_groups.new(name='G')
bpy.data.objects['S'].vertex_groups['G'].add([7, 10, 34], 0.1, 'REPLACE')
bpy.data.objects['S'].vertex_groups['G'].add([4, 30, 31], 0.2, 'REPLACE')
bpy.data.objects['S'].vertex_groups['G'].add([19, 24, 28], 0.3, 'REPLACE')
bpy.data.objects['S'].vertex_groups['G'].add([3, 16, 18, 22], 0.4, 'REPLACE')
bpy.data.objects['S'].vertex_groups['G'].add([23, 40], 0.5, 'REPLACE')
bpy.data.objects['S'].vertex_groups['G'].add([2], 0.6, 'REPLACE')
bpy.data.objects['S'].vertex_groups['G'].add([0, 6, 8, 29], 0.7, 'REPLACE')
bpy.data.objects['S'].vertex_groups['G'].add([13, 20, 21, 26], 0.8, 'REPLACE')
bpy.data.objects['S'].vertex_groups['G'].add([15, 36, 38, 41], 0.9, 'REPLACE')
bpy.data.objects['S'].vertex_groups['G'].add([9, 11, 27, 32, 37], 1.0, 'REPLACE')
bpy.data.objects['S'].vertex_groups.new(name='H')
bpy.data.objects['S'].vertex_groups['H'].add([9, 10, 15, 22, 28], 0.1, 'REPLACE')
bpy.data.objects['S'].vertex_groups['H'].add([6, 37], 0.2, 'REPLACE')
bpy.data.objects['S'].vertex_groups['H'].add([14, 23], 0.3, 'REPLACE')
bpy.data.objects['S'].vertex_groups['H'].add([17, 24, 27, 38], 0.4, 'REPLACE')
bpy.data.objects['S'].vertex_groups['H'].add([18, 19, 25], 0.5, 'REPLACE')
bpy.data.objects['S'].vertex_groups['H'].add([13, 21, 26, 31, 33, 39], 0.6, 'REPLACE')
bpy.data.objects['S'].vertex_groups['H'].add([2, 11], 0.7, 'REPLACE')
bpy.data.objects['S'].vertex_groups['H'].add([4], 0.8, 'REPLACE')
bpy.data.objects['S'].vertex_groups['H'].add([3, 16, 29, 32, 36, 40], 0.9, 'REPLACE')
bpy.data.objects['S'].vertex_groups['H'].add([1, 5, 8, 20], 1.0, 'REPLACE')
bpy.data.objects['S'].vertex_groups.new(name='K')
bpy.data.objects['S'].vertex_groups['K'].add([4, 10, 22, 29], 0.1, 'REPLACE')
bpy.data.objects['S'].vertex_groups['K'].add([5], 0.2, 'REPLACE')
bpy.data.objects['S'].vertex_groups['K'].add([3, 8, 38], 0.3, 'REPLACE')
bpy.data.objects['S'].vertex_groups['K'].add([2, 7, 16, 31, 35, 41], 0.4, 'REPLACE')
bpy.data.objects['S'].vertex_groups['K'].add([11, 15, 18, 25, 33], 0.5, 'REPLACE')
bpy.data.objects['S'].vertex_groups['K'].add([0, 36, 37, 40], 0.6, 'REPLACE')
bpy.data.objects['S'].vertex_groups['K'].add([9, 14, 32], 0.7, 'REPLACE')
bpy.data.objects['S'].vertex_groups['K'].add([6, 12, 30, 34], 0.8, 'REPLACE')
bpy.data.objects['S'].vertex_groups['K'].add([1, 19, 23, 26], 0.9, 'REPLACE')
bpy.data.objects['S'].vertex_groups['K'].add([13, 20, 24, 27, 28, 39], 1.0, 'REPLACE')
bpy.ops.mesh.primitive_grid_add(x_subdivisions=9, y_subdivisions=8, size=1.9, location=(0.03, -0.02, 0.1), rotation=(0.0, 0.0, 0.13))
bpy.context.object.name = 'DRep'
bpy.data.objects['DRep'].vertex_groups.new(name='G')
bpy.data.objects['DRep'].vertex_groups.new(name='H')
bpy.data.objects['DRep'].vertex_groups['G'].add([12, 27, 45, 56, 62, 70, 74], 0.1, 'REPLACE')
bpy.data.objects['DRep'].vertex_groups['G'].add([31, 32, 38, 47, 63, 82, 83], 0.2, 'REPLACE')
bpy.data.objects['DRep'].vertex_groups['G'].add([6, 16, 26, 28, 30, 35, 48, 49, 68, 78, 85, 88], 0.3, 'REPLACE')
bpy.data.objects['DRep'].vertex_groups['G'].add([4, 8, 13, 17, 19, 21, 42, 59], 0.4, 'REPLACE')
bpy.data.objects['DRep'].vertex_groups['G'].add([24, 36, 43, 61, 73], 0.5, 'REPLACE')
bpy.data.objects['DRep'].vertex_groups['G'].add([14, 55, 58], 0.6, 'REPLACE')
bpy.data.objects['DRep'].vertex_groups['G'].add([1, 40, 66, 87], 0.7, 'REPLACE')
bpy.data.objects['DRep'].vertex_groups['G'].add([29, 65, 67, 75], 0.8, 'REPLACE')
bpy.data.objects['DRep'].vertex_groups['G'].add([25, 39, 41, 51, 60, 69, 79, 86, 89], 0.9, 'REPLACE')
bpy.data.objects['DRep'].vertex_groups['G'].add([3, 7, 20, 33, 37, 46, 84], 1.0, 'REPLACE')
bpy.data.objects['DRep'].vertex_groups['H'].add([18, 49, 60, 84], 0.1, 'REPLACE')
bpy.data.objects['DRep'].vertex_groups['H'].add([5, 12, 30, 56, 65, 70], 0.2, 'REPLACE')
bpy.data.objects['DRep'].vertex_groups['H'].add([15, 48, 57, 80], 0.3, 'REPLACE')
bpy.data.objects['DRep'].vertex_groups['H'].add([41], 0.4, 'REPLACE')
bpy.data.objects['DRep'].vertex_groups['H'].add([17, 31, 74], 0.5, 'REPLACE')
bpy.data.objects['DRep'].vertex_groups['H'].add([35], 0.6, 'REPLACE')
bpy.data.objects['DRep'].vertex_groups['H'].add([7, 33, 34, 40, 44, 66, 67], 0.7, 'REPLACE')
bpy.data.objects['DRep'].vertex_groups['H'].add([27], 0.8, 'REPLACE')
bpy.data.objects['DRep'].vertex_groups['H'].add([46], 0.9, 'REPLACE')
bpy.data.objects['DRep'].vertex_groups['H'].add([4, 24, 71, 78], 1.0, 'REPLACE')
bpy.data.objects['DRep'].modifiers.new(name='DT', type='DATA_TRANSFER')
bpy.data.objects['DRep'].modifiers['DT'].object = bpy.data.objects['S']
bpy.data.objects['DRep'].modifiers['DT'].use_vert_data = True
bpy.data.objects['DRep'].modifiers['DT'].data_types_verts = {'VGROUP_WEIGHTS'}
bpy.data.objects['DRep'].modifiers['DT'].vert_mapping = 'POLYINTERP_NEAREST'
bpy.data.objects['DRep'].modifiers['DT'].mix_mode = 'REPLACE'
bpy.data.objects['DRep'].modifiers['DT'].mix_factor = 0.5
bpy.data.objects['DRep'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['DRep'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['DRep'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['DRep'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['DRep'].modifiers['Dz'].strength = 1.0
bpy.data.objects['DRep'].modifiers.new(name='Dx', type='DISPLACE')
bpy.data.objects['DRep'].modifiers['Dx'].vertex_group = 'H'
bpy.data.objects['DRep'].modifiers['Dx'].direction = 'X'
bpy.data.objects['DRep'].modifiers['Dx'].mid_level = 0.0
bpy.data.objects['DRep'].modifiers['Dx'].strength = 0.5
bpy.ops.mesh.primitive_grid_add(x_subdivisions=9, y_subdivisions=8, size=1.9, location=(0.03, -0.02, 0.1), rotation=(0.0, 0.0, 0.13))
bpy.context.object.name = 'DAbove'
bpy.data.objects['DAbove'].vertex_groups.new(name='G')
bpy.data.objects['DAbove'].vertex_groups.new(name='H')
bpy.data.objects['DAbove'].vertex_groups['G'].add([13, 21, 25, 62, 63, 84, 86], 0.1, 'REPLACE')
bpy.data.objects['DAbove'].vertex_groups['G'].add([3, 7, 9, 23, 31, 36, 37, 42, 49, 78], 0.2, 'REPLACE')
bpy.data.objects['DAbove'].vertex_groups['G'].add([0, 22, 29, 30, 33, 50, 57, 72, 82], 0.3, 'REPLACE')
bpy.data.objects['DAbove'].vertex_groups['G'].add([8, 14], 0.4, 'REPLACE')
bpy.data.objects['DAbove'].vertex_groups['G'].add([41, 48, 55, 88], 0.5, 'REPLACE')
bpy.data.objects['DAbove'].vertex_groups['G'].add([15, 17, 43, 77, 80, 81], 0.6, 'REPLACE')
bpy.data.objects['DAbove'].vertex_groups['G'].add([32, 46, 51, 59, 60, 65, 69, 74, 85], 0.7, 'REPLACE')
bpy.data.objects['DAbove'].vertex_groups['G'].add([26, 52, 76, 83], 0.8, 'REPLACE')
bpy.data.objects['DAbove'].vertex_groups['G'].add([2, 6, 19, 34, 35], 0.9, 'REPLACE')
bpy.data.objects['DAbove'].vertex_groups['G'].add([10, 18, 47, 58, 70, 87, 89], 1.0, 'REPLACE')
bpy.data.objects['DAbove'].vertex_groups['H'].add([26, 32, 34], 0.1, 'REPLACE')
bpy.data.objects['DAbove'].vertex_groups['H'].add([22, 23, 38, 48, 60, 81, 84], 0.2, 'REPLACE')
bpy.data.objects['DAbove'].vertex_groups['H'].add([5, 49, 69], 0.3, 'REPLACE')
bpy.data.objects['DAbove'].vertex_groups['H'].add([2, 8, 12, 14, 33, 37, 45, 52], 0.4, 'REPLACE')
bpy.data.objects['DAbove'].vertex_groups['H'].add([24, 40, 77, 79, 80], 0.5, 'REPLACE')
bpy.data.objects['DAbove'].vertex_groups['H'].add([13, 44, 47, 74, 76, 85], 0.6, 'REPLACE')
bpy.data.objects['DAbove'].vertex_groups['H'].add([35, 41], 0.7, 'REPLACE')
bpy.data.objects['DAbove'].vertex_groups['H'].add([0, 10, 19, 28, 88], 0.8, 'REPLACE')
bpy.data.objects['DAbove'].vertex_groups['H'].add([53, 56, 72, 86, 89], 0.9, 'REPLACE')
bpy.data.objects['DAbove'].vertex_groups['H'].add([17, 20, 67, 70], 1.0, 'REPLACE')
bpy.data.objects['DAbove'].modifiers.new(name='DT', type='DATA_TRANSFER')
bpy.data.objects['DAbove'].modifiers['DT'].object = bpy.data.objects['S']
bpy.data.objects['DAbove'].modifiers['DT'].use_vert_data = True
bpy.data.objects['DAbove'].modifiers['DT'].data_types_verts = {'VGROUP_WEIGHTS'}
bpy.data.objects['DAbove'].modifiers['DT'].vert_mapping = 'POLYINTERP_NEAREST'
bpy.data.objects['DAbove'].modifiers['DT'].mix_mode = 'ABOVE_THRESHOLD'
bpy.data.objects['DAbove'].modifiers['DT'].mix_factor = 0.45
bpy.data.objects['DAbove'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['DAbove'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['DAbove'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['DAbove'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['DAbove'].modifiers['Dz'].strength = 1.0
bpy.data.objects['DAbove'].modifiers.new(name='Dx', type='DISPLACE')
bpy.data.objects['DAbove'].modifiers['Dx'].vertex_group = 'H'
bpy.data.objects['DAbove'].modifiers['Dx'].direction = 'X'
bpy.data.objects['DAbove'].modifiers['Dx'].mid_level = 0.0
bpy.data.objects['DAbove'].modifiers['Dx'].strength = 0.5
bpy.ops.mesh.primitive_grid_add(x_subdivisions=9, y_subdivisions=8, size=1.9, location=(0.03, -0.02, 0.1), rotation=(0.0, 0.0, 0.13))
bpy.context.object.name = 'DBelow'
bpy.data.objects['DBelow'].vertex_groups.new(name='G')
bpy.data.objects['DBelow'].vertex_groups.new(name='H')
bpy.data.objects['DBelow'].vertex_groups['G'].add([3, 9, 15, 68, 78], 0.1, 'REPLACE')
bpy.data.objects['DBelow'].vertex_groups['G'].add([20, 36, 55, 62, 64], 0.2, 'REPLACE')
bpy.data.objects['DBelow'].vertex_groups['G'].add([5, 22, 37, 43, 83], 0.3, 'REPLACE')
bpy.data.objects['DBelow'].vertex_groups['G'].add([11, 21, 48, 52, 54, 59, 69, 80, 86], 0.4, 'REPLACE')
bpy.data.objects['DBelow'].vertex_groups['G'].add([10, 26, 38, 50, 60, 63], 0.5, 'REPLACE')
bpy.data.objects['DBelow'].vertex_groups['G'].add([18, 47, 61, 65, 73, 74, 85], 0.6, 'REPLACE')
bpy.data.objects['DBelow'].vertex_groups['G'].add([4, 30, 39, 46, 57, 75, 84], 0.7, 'REPLACE')
bpy.data.objects['DBelow'].vertex_groups['G'].add([0, 29, 41], 0.8, 'REPLACE')
bpy.data.objects['DBelow'].vertex_groups['G'].add([27, 45, 49, 53, 56, 72], 0.9, 'REPLACE')
bpy.data.objects['DBelow'].vertex_groups['G'].add([7, 24, 25, 40, 58], 1.0, 'REPLACE')
bpy.data.objects['DBelow'].vertex_groups['H'].add([10, 15, 27, 66], 0.1, 'REPLACE')
bpy.data.objects['DBelow'].vertex_groups['H'].add([2, 8, 32, 56, 72], 0.2, 'REPLACE')
bpy.data.objects['DBelow'].vertex_groups['H'].add([16, 34, 70, 83, 85, 87], 0.3, 'REPLACE')
bpy.data.objects['DBelow'].vertex_groups['H'].add([45, 51], 0.4, 'REPLACE')
bpy.data.objects['DBelow'].vertex_groups['H'].add([7, 41, 52, 67, 76, 86], 0.5, 'REPLACE')
bpy.data.objects['DBelow'].vertex_groups['H'].add([5, 11, 18, 26, 44], 0.6, 'REPLACE')
bpy.data.objects['DBelow'].vertex_groups['H'].add([1, 47, 60], 0.7, 'REPLACE')
bpy.data.objects['DBelow'].vertex_groups['H'].add([14, 33, 36, 48, 53, 57, 61, 74], 0.8, 'REPLACE')
bpy.data.objects['DBelow'].vertex_groups['H'].add([21, 50], 0.9, 'REPLACE')
bpy.data.objects['DBelow'].vertex_groups['H'].add([0, 9, 22, 42, 82, 89], 1.0, 'REPLACE')
bpy.data.objects['DBelow'].modifiers.new(name='DT', type='DATA_TRANSFER')
bpy.data.objects['DBelow'].modifiers['DT'].object = bpy.data.objects['S']
bpy.data.objects['DBelow'].modifiers['DT'].use_vert_data = True
bpy.data.objects['DBelow'].modifiers['DT'].data_types_verts = {'VGROUP_WEIGHTS'}
bpy.data.objects['DBelow'].modifiers['DT'].vert_mapping = 'POLYINTERP_NEAREST'
bpy.data.objects['DBelow'].modifiers['DT'].mix_mode = 'BELOW_THRESHOLD'
bpy.data.objects['DBelow'].modifiers['DT'].mix_factor = 0.55
bpy.data.objects['DBelow'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['DBelow'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['DBelow'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['DBelow'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['DBelow'].modifiers['Dz'].strength = 1.0
bpy.data.objects['DBelow'].modifiers.new(name='Dx', type='DISPLACE')
bpy.data.objects['DBelow'].modifiers['Dx'].vertex_group = 'H'
bpy.data.objects['DBelow'].modifiers['Dx'].direction = 'X'
bpy.data.objects['DBelow'].modifiers['Dx'].mid_level = 0.0
bpy.data.objects['DBelow'].modifiers['Dx'].strength = 0.5
bpy.ops.mesh.primitive_grid_add(x_subdivisions=9, y_subdivisions=8, size=1.9, location=(0.03, -0.02, 0.1), rotation=(0.0, 0.0, 0.13))
bpy.context.object.name = 'DMix'
bpy.data.objects['DMix'].vertex_groups.new(name='G')
bpy.data.objects['DMix'].vertex_groups.new(name='H')
bpy.data.objects['DMix'].vertex_groups['G'].add([19, 40, 42, 45, 71, 74, 84, 85, 88], 0.1, 'REPLACE')
bpy.data.objects['DMix'].vertex_groups['G'].add([5, 9, 12, 14, 36, 39, 64, 69], 0.2, 'REPLACE')
bpy.data.objects['DMix'].vertex_groups['G'].add([13, 18, 48, 54, 58, 65, 68, 75, 78], 0.3, 'REPLACE')
bpy.data.objects['DMix'].vertex_groups['G'].add([3, 22, 27, 28, 86], 0.4, 'REPLACE')
bpy.data.objects['DMix'].vertex_groups['G'].add([1, 10, 20, 25, 33, 79], 0.5, 'REPLACE')
bpy.data.objects['DMix'].vertex_groups['G'].add([4, 26, 83], 0.6, 'REPLACE')
bpy.data.objects['DMix'].vertex_groups['G'].add([21, 31, 38, 59, 70, 76, 82], 0.7, 'REPLACE')
bpy.data.objects['DMix'].vertex_groups['G'].add([32, 46, 47, 50, 56, 87], 0.8, 'REPLACE')
bpy.data.objects['DMix'].vertex_groups['G'].add([8, 16, 17, 41, 72, 80], 0.9, 'REPLACE')
bpy.data.objects['DMix'].vertex_groups['G'].add([2, 61], 1.0, 'REPLACE')
bpy.data.objects['DMix'].vertex_groups['H'].add([5, 73, 82, 86], 0.1, 'REPLACE')
bpy.data.objects['DMix'].vertex_groups['H'].add([10, 55], 0.2, 'REPLACE')
bpy.data.objects['DMix'].vertex_groups['H'].add([15, 31, 48, 49, 83], 0.3, 'REPLACE')
bpy.data.objects['DMix'].vertex_groups['H'].add([30, 38, 63, 74], 0.4, 'REPLACE')
bpy.data.objects['DMix'].vertex_groups['H'].add([39, 62, 76, 81], 0.5, 'REPLACE')
bpy.data.objects['DMix'].vertex_groups['H'].add([0, 9, 29, 46, 50, 51], 0.6, 'REPLACE')
bpy.data.objects['DMix'].vertex_groups['H'].add([17, 44], 0.7, 'REPLACE')
bpy.data.objects['DMix'].vertex_groups['H'].add([2, 23, 75, 78], 0.8, 'REPLACE')
bpy.data.objects['DMix'].vertex_groups['H'].add([12, 27, 54, 84], 0.9, 'REPLACE')
bpy.data.objects['DMix'].vertex_groups['H'].add([4, 18, 68, 87], 1.0, 'REPLACE')
bpy.data.objects['DMix'].modifiers.new(name='DT', type='DATA_TRANSFER')
bpy.data.objects['DMix'].modifiers['DT'].object = bpy.data.objects['S']
bpy.data.objects['DMix'].modifiers['DT'].use_vert_data = True
bpy.data.objects['DMix'].modifiers['DT'].data_types_verts = {'VGROUP_WEIGHTS'}
bpy.data.objects['DMix'].modifiers['DT'].vert_mapping = 'POLYINTERP_NEAREST'
bpy.data.objects['DMix'].modifiers['DT'].mix_mode = 'MIX'
bpy.data.objects['DMix'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['DMix'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['DMix'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['DMix'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['DMix'].modifiers['Dz'].strength = 1.0
bpy.data.objects['DMix'].modifiers.new(name='Dx', type='DISPLACE')
bpy.data.objects['DMix'].modifiers['Dx'].vertex_group = 'H'
bpy.data.objects['DMix'].modifiers['Dx'].direction = 'X'
bpy.data.objects['DMix'].modifiers['Dx'].mid_level = 0.0
bpy.data.objects['DMix'].modifiers['Dx'].strength = 0.5
bpy.ops.mesh.primitive_grid_add(x_subdivisions=9, y_subdivisions=8, size=1.9, location=(0.03, -0.02, 0.1), rotation=(0.0, 0.0, 0.13))
bpy.context.object.name = 'DAdd'
bpy.data.objects['DAdd'].vertex_groups.new(name='G')
bpy.data.objects['DAdd'].vertex_groups.new(name='H')
bpy.data.objects['DAdd'].vertex_groups['G'].add([10, 21, 23, 29, 57, 67, 70, 86], 0.1, 'REPLACE')
bpy.data.objects['DAdd'].vertex_groups['G'].add([3, 7, 19, 50, 62, 75, 88], 0.2, 'REPLACE')
bpy.data.objects['DAdd'].vertex_groups['G'].add([16, 22, 27, 43, 63, 68, 83, 89], 0.3, 'REPLACE')
bpy.data.objects['DAdd'].vertex_groups['G'].add([5, 46, 55, 60, 69, 79], 0.4, 'REPLACE')
bpy.data.objects['DAdd'].vertex_groups['G'].add([18, 72], 0.5, 'REPLACE')
bpy.data.objects['DAdd'].vertex_groups['G'].add([26, 49, 54, 71, 84], 0.6, 'REPLACE')
bpy.data.objects['DAdd'].vertex_groups['G'].add([17, 30, 32, 42, 56, 58, 78, 81, 87], 0.7, 'REPLACE')
bpy.data.objects['DAdd'].vertex_groups['G'].add([35, 40, 51, 64, 76], 0.8, 'REPLACE')
bpy.data.objects['DAdd'].vertex_groups['G'].add([1, 8, 34, 47, 77], 0.9, 'REPLACE')
bpy.data.objects['DAdd'].vertex_groups['G'].add([2, 4, 12, 20, 25, 66, 73, 74], 1.0, 'REPLACE')
bpy.data.objects['DAdd'].vertex_groups['H'].add([1, 10, 19, 27, 31, 41, 57, 88], 0.1, 'REPLACE')
bpy.data.objects['DAdd'].vertex_groups['H'].add([14, 29, 65, 86], 0.2, 'REPLACE')
bpy.data.objects['DAdd'].vertex_groups['H'].add([12, 25, 70, 74], 0.3, 'REPLACE')
bpy.data.objects['DAdd'].vertex_groups['H'].add([22, 30, 35, 38, 48, 75, 79, 89], 0.4, 'REPLACE')
bpy.data.objects['DAdd'].vertex_groups['H'].add([26, 40, 46, 69], 0.5, 'REPLACE')
bpy.data.objects['DAdd'].vertex_groups['H'].add([2, 5, 7, 9, 33, 34, 36, 42, 59], 0.6, 'REPLACE')
bpy.data.objects['DAdd'].vertex_groups['H'].add([20, 60, 87], 0.7, 'REPLACE')
bpy.data.objects['DAdd'].vertex_groups['H'].add([3, 4, 11, 21, 44, 50, 66, 67, 81], 0.8, 'REPLACE')
bpy.data.objects['DAdd'].vertex_groups['H'].add([6, 52, 53, 56, 76], 0.9, 'REPLACE')
bpy.data.objects['DAdd'].vertex_groups['H'].add([23, 72], 1.0, 'REPLACE')
bpy.data.objects['DAdd'].modifiers.new(name='DT', type='DATA_TRANSFER')
bpy.data.objects['DAdd'].modifiers['DT'].object = bpy.data.objects['S']
bpy.data.objects['DAdd'].modifiers['DT'].use_vert_data = True
bpy.data.objects['DAdd'].modifiers['DT'].data_types_verts = {'VGROUP_WEIGHTS'}
bpy.data.objects['DAdd'].modifiers['DT'].vert_mapping = 'POLYINTERP_NEAREST'
bpy.data.objects['DAdd'].modifiers['DT'].mix_mode = 'ADD'
bpy.data.objects['DAdd'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['DAdd'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['DAdd'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['DAdd'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['DAdd'].modifiers['Dz'].strength = 1.0
bpy.data.objects['DAdd'].modifiers.new(name='Dx', type='DISPLACE')
bpy.data.objects['DAdd'].modifiers['Dx'].vertex_group = 'H'
bpy.data.objects['DAdd'].modifiers['Dx'].direction = 'X'
bpy.data.objects['DAdd'].modifiers['Dx'].mid_level = 0.0
bpy.data.objects['DAdd'].modifiers['Dx'].strength = 0.5
bpy.ops.mesh.primitive_grid_add(x_subdivisions=9, y_subdivisions=8, size=1.9, location=(0.03, -0.02, 0.1), rotation=(0.0, 0.0, 0.13))
bpy.context.object.name = 'DSub'
bpy.data.objects['DSub'].vertex_groups.new(name='G')
bpy.data.objects['DSub'].vertex_groups.new(name='H')
bpy.data.objects['DSub'].vertex_groups['G'].add([2, 12, 24, 33, 66, 69, 75], 0.1, 'REPLACE')
bpy.data.objects['DSub'].vertex_groups['G'].add([30, 34, 51, 52, 57, 61], 0.2, 'REPLACE')
bpy.data.objects['DSub'].vertex_groups['G'].add([6, 10, 14, 55, 70, 80], 0.3, 'REPLACE')
bpy.data.objects['DSub'].vertex_groups['G'].add([13, 18, 83, 87], 0.4, 'REPLACE')
bpy.data.objects['DSub'].vertex_groups['G'].add([11, 17, 22, 28, 29, 36, 67, 78], 0.5, 'REPLACE')
bpy.data.objects['DSub'].vertex_groups['G'].add([3, 23, 46, 50, 60, 73, 74, 82], 0.6, 'REPLACE')
bpy.data.objects['DSub'].vertex_groups['G'].add([0, 20, 21, 37, 63, 84], 0.7, 'REPLACE')
bpy.data.objects['DSub'].vertex_groups['G'].add([5, 38, 53, 56, 58, 62, 64, 88], 0.8, 'REPLACE')
bpy.data.objects['DSub'].vertex_groups['G'].add([9, 43], 0.9, 'REPLACE')
bpy.data.objects['DSub'].vertex_groups['G'].add([16, 26, 32, 41, 42, 44, 65, 72, 76, 77], 1.0, 'REPLACE')
bpy.data.objects['DSub'].vertex_groups['H'].add([6, 17, 21, 58], 0.1, 'REPLACE')
bpy.data.objects['DSub'].vertex_groups['H'].add([80], 0.2, 'REPLACE')
bpy.data.objects['DSub'].vertex_groups['H'].add([11, 29, 43, 49, 76], 0.3, 'REPLACE')
bpy.data.objects['DSub'].vertex_groups['H'].add([22, 54, 60], 0.4, 'REPLACE')
bpy.data.objects['DSub'].vertex_groups['H'].add([71], 0.5, 'REPLACE')
bpy.data.objects['DSub'].vertex_groups['H'].add([15, 19, 40, 61, 75], 0.6, 'REPLACE')
bpy.data.objects['DSub'].vertex_groups['H'].add([0, 10, 57, 65, 66, 69, 84], 0.7, 'REPLACE')
bpy.data.objects['DSub'].vertex_groups['H'].add([1, 26, 64, 78], 0.8, 'REPLACE')
bpy.data.objects['DSub'].vertex_groups['H'].add([8, 18, 39, 55, 74, 81, 82], 0.9, 'REPLACE')
bpy.data.objects['DSub'].vertex_groups['H'].add([14, 73], 1.0, 'REPLACE')
bpy.data.objects['DSub'].modifiers.new(name='DT', type='DATA_TRANSFER')
bpy.data.objects['DSub'].modifiers['DT'].object = bpy.data.objects['S']
bpy.data.objects['DSub'].modifiers['DT'].use_vert_data = True
bpy.data.objects['DSub'].modifiers['DT'].data_types_verts = {'VGROUP_WEIGHTS'}
bpy.data.objects['DSub'].modifiers['DT'].vert_mapping = 'POLYINTERP_NEAREST'
bpy.data.objects['DSub'].modifiers['DT'].mix_mode = 'SUB'
bpy.data.objects['DSub'].modifiers['DT'].mix_factor = 0.8
bpy.data.objects['DSub'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['DSub'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['DSub'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['DSub'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['DSub'].modifiers['Dz'].strength = 1.0
bpy.data.objects['DSub'].modifiers.new(name='Dx', type='DISPLACE')
bpy.data.objects['DSub'].modifiers['Dx'].vertex_group = 'H'
bpy.data.objects['DSub'].modifiers['Dx'].direction = 'X'
bpy.data.objects['DSub'].modifiers['Dx'].mid_level = 0.0
bpy.data.objects['DSub'].modifiers['Dx'].strength = 0.5
bpy.ops.mesh.primitive_grid_add(x_subdivisions=9, y_subdivisions=8, size=1.9, location=(0.03, -0.02, 0.1), rotation=(0.0, 0.0, 0.13))
bpy.context.object.name = 'DMul'
bpy.data.objects['DMul'].vertex_groups.new(name='G')
bpy.data.objects['DMul'].vertex_groups.new(name='H')
bpy.data.objects['DMul'].vertex_groups['G'].add([23, 42, 62, 66, 67, 73, 84], 0.1, 'REPLACE')
bpy.data.objects['DMul'].vertex_groups['G'].add([7, 10, 16, 21, 22, 45, 60, 76, 82], 0.2, 'REPLACE')
bpy.data.objects['DMul'].vertex_groups['G'].add([18, 25, 26, 31, 33, 57, 75, 77, 78, 81], 0.3, 'REPLACE')
bpy.data.objects['DMul'].vertex_groups['G'].add([17, 35, 41, 46, 49, 69, 80], 0.4, 'REPLACE')
bpy.data.objects['DMul'].vertex_groups['G'].add([4, 14, 19, 30, 48], 0.5, 'REPLACE')
bpy.data.objects['DMul'].vertex_groups['G'].add([13, 15, 24, 43, 50, 54], 0.6, 'REPLACE')
bpy.data.objects['DMul'].vertex_groups['G'].add([0, 6, 40, 79, 87], 0.7, 'REPLACE')
bpy.data.objects['DMul'].vertex_groups['G'].add([32, 37, 56, 59, 64, 89], 0.8, 'REPLACE')
bpy.data.objects['DMul'].vertex_groups['G'].add([9, 51, 53, 68, 70, 88], 0.9, 'REPLACE')
bpy.data.objects['DMul'].vertex_groups['G'].add([1, 2, 83, 85], 1.0, 'REPLACE')
bpy.data.objects['DMul'].vertex_groups['H'].add([34, 39, 46, 71, 75], 0.1, 'REPLACE')
bpy.data.objects['DMul'].vertex_groups['H'].add([22, 36, 52, 62, 77], 0.2, 'REPLACE')
bpy.data.objects['DMul'].vertex_groups['H'].add([29, 63], 0.3, 'REPLACE')
bpy.data.objects['DMul'].vertex_groups['H'].add([11, 24, 45, 73], 0.4, 'REPLACE')
bpy.data.objects['DMul'].vertex_groups['H'].add([0, 13, 53, 57], 0.5, 'REPLACE')
bpy.data.objects['DMul'].vertex_groups['H'].add([12, 47, 48], 0.6, 'REPLACE')
bpy.data.objects['DMul'].vertex_groups['H'].add([6, 74], 0.7, 'REPLACE')
bpy.data.objects['DMul'].vertex_groups['H'].add([17, 33, 35, 56, 59, 67, 72, 81], 0.8, 'REPLACE')
bpy.data.objects['DMul'].vertex_groups['H'].add([15, 16, 25, 89], 0.9, 'REPLACE')
bpy.data.objects['DMul'].vertex_groups['H'].add([2, 32, 37, 40, 41, 83, 85, 88], 1.0, 'REPLACE')
bpy.data.objects['DMul'].modifiers.new(name='DT', type='DATA_TRANSFER')
bpy.data.objects['DMul'].modifiers['DT'].object = bpy.data.objects['S']
bpy.data.objects['DMul'].modifiers['DT'].use_vert_data = True
bpy.data.objects['DMul'].modifiers['DT'].data_types_verts = {'VGROUP_WEIGHTS'}
bpy.data.objects['DMul'].modifiers['DT'].vert_mapping = 'POLYINTERP_NEAREST'
bpy.data.objects['DMul'].modifiers['DT'].mix_mode = 'MUL'
bpy.data.objects['DMul'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['DMul'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['DMul'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['DMul'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['DMul'].modifiers['Dz'].strength = 1.0
bpy.data.objects['DMul'].modifiers.new(name='Dx', type='DISPLACE')
bpy.data.objects['DMul'].modifiers['Dx'].vertex_group = 'H'
bpy.data.objects['DMul'].modifiers['Dx'].direction = 'X'
bpy.data.objects['DMul'].modifiers['Dx'].mid_level = 0.0
bpy.data.objects['DMul'].modifiers['Dx'].strength = 0.5
bpy.ops.mesh.primitive_grid_add(x_subdivisions=9, y_subdivisions=8, size=1.9, location=(0.03, -0.02, 0.1), rotation=(0.0, 0.0, 0.13))
bpy.context.object.name = 'DMask'
bpy.data.objects['DMask'].vertex_groups.new(name='G')
bpy.data.objects['DMask'].vertex_groups.new(name='H')
bpy.data.objects['DMask'].vertex_groups['G'].add([36, 71, 74], 0.1, 'REPLACE')
bpy.data.objects['DMask'].vertex_groups['G'].add([11, 16, 34, 35, 84], 0.2, 'REPLACE')
bpy.data.objects['DMask'].vertex_groups['G'].add([10, 17, 22, 25, 37, 48, 57, 59, 78], 0.3, 'REPLACE')
bpy.data.objects['DMask'].vertex_groups['G'].add([15, 32, 38, 58, 63, 79, 89], 0.4, 'REPLACE')
bpy.data.objects['DMask'].vertex_groups['G'].add([9, 20, 39, 61, 62, 66], 0.5, 'REPLACE')
bpy.data.objects['DMask'].vertex_groups['G'].add([2, 27, 40, 43, 47, 56, 64, 83, 86], 0.6, 'REPLACE')
bpy.data.objects['DMask'].vertex_groups['G'].add([8, 31], 0.7, 'REPLACE')
bpy.data.objects['DMask'].vertex_groups['G'].add([6, 18, 23, 29, 41, 46, 77, 85], 0.8, 'REPLACE')
bpy.data.objects['DMask'].vertex_groups['G'].add([5, 14, 21, 30, 44, 50, 53, 68, 73, 87], 0.9, 'REPLACE')
bpy.data.objects['DMask'].vertex_groups['G'].add([1, 13, 28, 49, 70, 72, 81, 82, 88], 1.0, 'REPLACE')
bpy.data.objects['DMask'].vertex_groups['H'].add([3, 20, 36], 0.1, 'REPLACE')
bpy.data.objects['DMask'].vertex_groups['H'].add([15, 19, 30, 71, 73], 0.2, 'REPLACE')
bpy.data.objects['DMask'].vertex_groups['H'].add([16, 29, 58, 59, 70, 74, 76, 78, 89], 0.3, 'REPLACE')
bpy.data.objects['DMask'].vertex_groups['H'].add([2, 39, 86], 0.4, 'REPLACE')
bpy.data.objects['DMask'].vertex_groups['H'].add([9, 24, 43, 52, 56], 0.5, 'REPLACE')
bpy.data.objects['DMask'].vertex_groups['H'].add([31, 48, 61, 84], 0.6, 'REPLACE')
bpy.data.objects['DMask'].vertex_groups['H'].add([23, 33, 37, 80], 0.7, 'REPLACE')
bpy.data.objects['DMask'].vertex_groups['H'].add([0, 26, 53, 65, 75], 0.8, 'REPLACE')
bpy.data.objects['DMask'].vertex_groups['H'].add([42], 0.9, 'REPLACE')
bpy.data.objects['DMask'].vertex_groups['H'].add([4, 51], 1.0, 'REPLACE')
bpy.data.objects['DMask'].vertex_groups.new(name='M')
bpy.data.objects['DMask'].vertex_groups['M'].add([1, 9, 12, 25, 37, 38, 56, 59, 74], 0.1, 'REPLACE')
bpy.data.objects['DMask'].vertex_groups['M'].add([6, 8, 19, 23, 48, 63, 73, 78, 88], 0.2, 'REPLACE')
bpy.data.objects['DMask'].vertex_groups['M'].add([18, 21, 45, 50, 61, 70, 71], 0.3, 'REPLACE')
bpy.data.objects['DMask'].vertex_groups['M'].add([2, 10, 39, 42, 49, 89], 0.4, 'REPLACE')
bpy.data.objects['DMask'].vertex_groups['M'].add([13, 31, 43, 44, 66, 69, 75, 84], 0.5, 'REPLACE')
bpy.data.objects['DMask'].vertex_groups['M'].add([4, 30, 34, 53, 57, 58, 65, 76, 80], 0.6, 'REPLACE')
bpy.data.objects['DMask'].vertex_groups['M'].add([7, 27, 83, 85], 0.7, 'REPLACE')
bpy.data.objects['DMask'].vertex_groups['M'].add([15, 16, 24, 26, 35, 46, 47, 52, 67, 81, 86], 0.9, 'REPLACE')
bpy.data.objects['DMask'].vertex_groups['M'].add([22, 28, 29, 54, 62, 77], 1.0, 'REPLACE')
bpy.data.objects['DMask'].modifiers.new(name='DT', type='DATA_TRANSFER')
bpy.data.objects['DMask'].modifiers['DT'].object = bpy.data.objects['S']
bpy.data.objects['DMask'].modifiers['DT'].use_vert_data = True
bpy.data.objects['DMask'].modifiers['DT'].data_types_verts = {'VGROUP_WEIGHTS'}
bpy.data.objects['DMask'].modifiers['DT'].vert_mapping = 'POLYINTERP_NEAREST'
bpy.data.objects['DMask'].modifiers['DT'].vertex_group = 'M'
bpy.data.objects['DMask'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['DMask'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['DMask'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['DMask'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['DMask'].modifiers['Dz'].strength = 1.0
bpy.data.objects['DMask'].modifiers.new(name='Dx', type='DISPLACE')
bpy.data.objects['DMask'].modifiers['Dx'].vertex_group = 'H'
bpy.data.objects['DMask'].modifiers['Dx'].direction = 'X'
bpy.data.objects['DMask'].modifiers['Dx'].mid_level = 0.0
bpy.data.objects['DMask'].modifiers['Dx'].strength = 0.5
bpy.ops.mesh.primitive_grid_add(x_subdivisions=9, y_subdivisions=8, size=1.9, location=(0.03, -0.02, 0.1), rotation=(0.0, 0.0, 0.13))
bpy.context.object.name = 'DMaskInv'
bpy.data.objects['DMaskInv'].vertex_groups.new(name='G')
bpy.data.objects['DMaskInv'].vertex_groups.new(name='H')
bpy.data.objects['DMaskInv'].vertex_groups['G'].add([12, 35, 65, 87], 0.1, 'REPLACE')
bpy.data.objects['DMaskInv'].vertex_groups['G'].add([0, 2, 21, 43, 58, 61, 72, 79], 0.2, 'REPLACE')
bpy.data.objects['DMaskInv'].vertex_groups['G'].add([22, 41, 46], 0.3, 'REPLACE')
bpy.data.objects['DMaskInv'].vertex_groups['G'].add([53, 85], 0.4, 'REPLACE')
bpy.data.objects['DMaskInv'].vertex_groups['G'].add([1, 8, 25, 67, 78, 89], 0.5, 'REPLACE')
bpy.data.objects['DMaskInv'].vertex_groups['G'].add([11, 17, 24, 38, 45, 81], 0.6, 'REPLACE')
bpy.data.objects['DMaskInv'].vertex_groups['G'].add([6, 13, 28, 37, 42, 52, 54, 82], 0.7, 'REPLACE')
bpy.data.objects['DMaskInv'].vertex_groups['G'].add([60, 74, 77], 0.8, 'REPLACE')
bpy.data.objects['DMaskInv'].vertex_groups['G'].add([5, 9, 15, 16, 27, 31, 32, 50, 66], 0.9, 'REPLACE')
bpy.data.objects['DMaskInv'].vertex_groups['G'].add([3, 14, 19, 34, 40, 51, 56, 57, 64, 70], 1.0, 'REPLACE')
bpy.data.objects['DMaskInv'].vertex_groups['H'].add([2, 7, 13, 24, 26, 40, 48, 61], 0.1, 'REPLACE')
bpy.data.objects['DMaskInv'].vertex_groups['H'].add([3, 56, 74, 77], 0.2, 'REPLACE')
bpy.data.objects['DMaskInv'].vertex_groups['H'].add([14, 31, 38, 83], 0.3, 'REPLACE')
bpy.data.objects['DMaskInv'].vertex_groups['H'].add([4, 22, 59, 84], 0.4, 'REPLACE')
bpy.data.objects['DMaskInv'].vertex_groups['H'].add([62, 68, 70, 78, 80, 85], 0.5, 'REPLACE')
bpy.data.objects['DMaskInv'].vertex_groups['H'].add([1, 73], 0.6, 'REPLACE')
bpy.data.objects['DMaskInv'].vertex_groups['H'].add([6, 28, 32, 44, 72, 75, 88], 0.7, 'REPLACE')
bpy.data.objects['DMaskInv'].vertex_groups['H'].add([25, 37, 60], 0.8, 'REPLACE')
bpy.data.objects['DMaskInv'].vertex_groups['H'].add([41, 63, 81, 82, 86], 0.9, 'REPLACE')
bpy.data.objects['DMaskInv'].vertex_groups['H'].add([5, 12, 15, 27, 53, 79], 1.0, 'REPLACE')
bpy.data.objects['DMaskInv'].vertex_groups.new(name='M')
bpy.data.objects['DMaskInv'].vertex_groups['M'].add([20, 23, 24, 29, 46, 74, 77], 0.1, 'REPLACE')
bpy.data.objects['DMaskInv'].vertex_groups['M'].add([7, 17, 21, 39, 52, 58, 86], 0.2, 'REPLACE')
bpy.data.objects['DMaskInv'].vertex_groups['M'].add([1, 3, 4, 5, 25, 30, 31, 32, 34, 35, 36, 40, 45, 55, 61, 63, 67, 76, 81], 0.3, 'REPLACE')
bpy.data.objects['DMaskInv'].vertex_groups['M'].add([2, 15, 64, 78], 0.4, 'REPLACE')
bpy.data.objects['DMaskInv'].vertex_groups['M'].add([0, 9, 26, 42, 54], 0.5, 'REPLACE')
bpy.data.objects['DMaskInv'].vertex_groups['M'].add([18, 37, 38, 41, 57, 68, 69, 72, 87, 89], 0.6, 'REPLACE')
bpy.data.objects['DMaskInv'].vertex_groups['M'].add([8, 44, 47, 53, 59, 60], 0.7, 'REPLACE')
bpy.data.objects['DMaskInv'].vertex_groups['M'].add([14, 19, 27, 56, 66, 79, 83, 88], 0.8, 'REPLACE')
bpy.data.objects['DMaskInv'].vertex_groups['M'].add([6, 13, 62, 70, 80, 85], 0.9, 'REPLACE')
bpy.data.objects['DMaskInv'].vertex_groups['M'].add([11, 22, 49, 51, 75], 1.0, 'REPLACE')
bpy.data.objects['DMaskInv'].modifiers.new(name='DT', type='DATA_TRANSFER')
bpy.data.objects['DMaskInv'].modifiers['DT'].object = bpy.data.objects['S']
bpy.data.objects['DMaskInv'].modifiers['DT'].use_vert_data = True
bpy.data.objects['DMaskInv'].modifiers['DT'].data_types_verts = {'VGROUP_WEIGHTS'}
bpy.data.objects['DMaskInv'].modifiers['DT'].vert_mapping = 'POLYINTERP_NEAREST'
bpy.data.objects['DMaskInv'].modifiers['DT'].vertex_group = 'M'
bpy.data.objects['DMaskInv'].modifiers['DT'].invert_vertex_group = True
bpy.data.objects['DMaskInv'].modifiers['DT'].mix_mode = 'ADD'
bpy.data.objects['DMaskInv'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['DMaskInv'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['DMaskInv'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['DMaskInv'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['DMaskInv'].modifiers['Dz'].strength = 1.0
bpy.data.objects['DMaskInv'].modifiers.new(name='Dx', type='DISPLACE')
bpy.data.objects['DMaskInv'].modifiers['Dx'].vertex_group = 'H'
bpy.data.objects['DMaskInv'].modifiers['Dx'].direction = 'X'
bpy.data.objects['DMaskInv'].modifiers['Dx'].mid_level = 0.0
bpy.data.objects['DMaskInv'].modifiers['Dx'].strength = 0.5
bpy.ops.mesh.primitive_grid_add(x_subdivisions=9, y_subdivisions=8, size=1.9, location=(0.03, -0.02, 0.1), rotation=(0.0, 0.0, 0.13))
bpy.context.object.name = 'DIndex'
bpy.data.objects['DIndex'].vertex_groups.new(name='X')
bpy.data.objects['DIndex'].vertex_groups.new(name='Y')
bpy.data.objects['DIndex'].vertex_groups.new(name='Z')
bpy.data.objects['DIndex'].vertex_groups.new(name='W')
bpy.data.objects['DIndex'].vertex_groups['X'].add([15, 24, 25, 45, 71, 72], 0.1, 'REPLACE')
bpy.data.objects['DIndex'].vertex_groups['X'].add([5, 9, 10, 26, 42, 47, 50, 80], 0.2, 'REPLACE')
bpy.data.objects['DIndex'].vertex_groups['X'].add([23, 28, 30, 53, 87], 0.3, 'REPLACE')
bpy.data.objects['DIndex'].vertex_groups['X'].add([56, 75, 81, 86], 0.4, 'REPLACE')
bpy.data.objects['DIndex'].vertex_groups['X'].add([1, 58, 62, 77], 0.5, 'REPLACE')
bpy.data.objects['DIndex'].vertex_groups['X'].add([11, 13, 18, 22, 59, 82], 0.6, 'REPLACE')
bpy.data.objects['DIndex'].vertex_groups['X'].add([0, 3, 7, 8, 14, 19, 39, 40, 48, 66], 0.7, 'REPLACE')
bpy.data.objects['DIndex'].vertex_groups['X'].add([4, 29, 49, 63, 70, 84, 89], 0.8, 'REPLACE')
bpy.data.objects['DIndex'].vertex_groups['X'].add([37, 54, 61, 67, 85], 0.9, 'REPLACE')
bpy.data.objects['DIndex'].vertex_groups['X'].add([6, 12, 17, 35, 55, 69, 88], 1.0, 'REPLACE')
bpy.data.objects['DIndex'].vertex_groups['Y'].add([39, 52, 70, 82], 0.1, 'REPLACE')
bpy.data.objects['DIndex'].vertex_groups['Y'].add([20, 44, 56, 59, 77], 0.2, 'REPLACE')
bpy.data.objects['DIndex'].vertex_groups['Y'].add([35, 48, 68, 76], 0.3, 'REPLACE')
bpy.data.objects['DIndex'].vertex_groups['Y'].add([2, 7, 50, 51, 55, 61], 0.4, 'REPLACE')
bpy.data.objects['DIndex'].vertex_groups['Y'].add([34, 40, 83], 0.5, 'REPLACE')
bpy.data.objects['DIndex'].vertex_groups['Y'].add([9, 28, 29, 58], 0.6, 'REPLACE')
bpy.data.objects['DIndex'].vertex_groups['Y'].add([0, 6, 13, 15, 25], 0.7, 'REPLACE')
bpy.data.objects['DIndex'].vertex_groups['Y'].add([86, 87], 0.8, 'REPLACE')
bpy.data.objects['DIndex'].vertex_groups['Y'].add([23, 41, 53, 85], 0.9, 'REPLACE')
bpy.data.objects['DIndex'].vertex_groups['Y'].add([64, 79], 1.0, 'REPLACE')
bpy.data.objects['DIndex'].modifiers.new(name='DT', type='DATA_TRANSFER')
bpy.data.objects['DIndex'].modifiers['DT'].object = bpy.data.objects['S']
bpy.data.objects['DIndex'].modifiers['DT'].use_vert_data = True
bpy.data.objects['DIndex'].modifiers['DT'].data_types_verts = {'VGROUP_WEIGHTS'}
bpy.data.objects['DIndex'].modifiers['DT'].vert_mapping = 'POLYINTERP_NEAREST'
bpy.data.objects['DIndex'].modifiers['DT'].layers_vgroup_select_dst = 'INDEX'
bpy.data.objects['DIndex'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['DIndex'].modifiers['Dz'].vertex_group = 'X'
bpy.data.objects['DIndex'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['DIndex'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['DIndex'].modifiers['Dz'].strength = 1.0
bpy.data.objects['DIndex'].modifiers.new(name='Dx', type='DISPLACE')
bpy.data.objects['DIndex'].modifiers['Dx'].vertex_group = 'Y'
bpy.data.objects['DIndex'].modifiers['Dx'].direction = 'X'
bpy.data.objects['DIndex'].modifiers['Dx'].mid_level = 0.0
bpy.data.objects['DIndex'].modifiers['Dx'].strength = 0.5
bpy.data.objects['DIndex'].modifiers.new(name='Dy', type='DISPLACE')
bpy.data.objects['DIndex'].modifiers['Dy'].vertex_group = 'Z'
bpy.data.objects['DIndex'].modifiers['Dy'].direction = 'Y'
bpy.data.objects['DIndex'].modifiers['Dy'].mid_level = 0.0
bpy.data.objects['DIndex'].modifiers['Dy'].strength = 0.25
bpy.ops.mesh.primitive_grid_add(x_subdivisions=9, y_subdivisions=8, size=1.9, location=(0.03, -0.02, 0.1), rotation=(0.0, 0.0, 0.13))
bpy.context.object.name = 'DMaxDist'
bpy.data.objects['DMaxDist'].vertex_groups.new(name='G')
bpy.data.objects['DMaxDist'].vertex_groups.new(name='H')
bpy.data.objects['DMaxDist'].vertex_groups['G'].add([4, 23, 45], 0.1, 'REPLACE')
bpy.data.objects['DMaxDist'].vertex_groups['G'].add([7, 15, 34, 52, 81], 0.2, 'REPLACE')
bpy.data.objects['DMaxDist'].vertex_groups['G'].add([54, 63, 66, 72], 0.3, 'REPLACE')
bpy.data.objects['DMaxDist'].vertex_groups['G'].add([11, 37, 38, 70, 74, 77], 0.4, 'REPLACE')
bpy.data.objects['DMaxDist'].vertex_groups['G'].add([0, 13, 14, 18, 19, 22, 24, 35, 42, 60, 78, 85, 87], 0.5, 'REPLACE')
bpy.data.objects['DMaxDist'].vertex_groups['G'].add([20, 33, 36, 55, 80], 0.6, 'REPLACE')
bpy.data.objects['DMaxDist'].vertex_groups['G'].add([8, 28, 40, 59, 62, 64, 82, 89], 0.7, 'REPLACE')
bpy.data.objects['DMaxDist'].vertex_groups['G'].add([9, 17, 26, 32, 46, 71, 73], 0.8, 'REPLACE')
bpy.data.objects['DMaxDist'].vertex_groups['G'].add([16, 21, 30, 31, 47, 56, 57, 69, 76], 0.9, 'REPLACE')
bpy.data.objects['DMaxDist'].vertex_groups['G'].add([6, 10, 12, 50], 1.0, 'REPLACE')
bpy.data.objects['DMaxDist'].vertex_groups['H'].add([8, 21, 29, 57, 68, 86], 0.1, 'REPLACE')
bpy.data.objects['DMaxDist'].vertex_groups['H'].add([22, 41, 49, 60, 71], 0.2, 'REPLACE')
bpy.data.objects['DMaxDist'].vertex_groups['H'].add([4, 83], 0.3, 'REPLACE')
bpy.data.objects['DMaxDist'].vertex_groups['H'].add([12, 38, 42, 45, 63], 0.4, 'REPLACE')
bpy.data.objects['DMaxDist'].vertex_groups['H'].add([15, 17, 48, 58, 85], 0.5, 'REPLACE')
bpy.data.objects['DMaxDist'].vertex_groups['H'].add([13, 23, 28, 37, 65], 0.6, 'REPLACE')
bpy.data.objects['DMaxDist'].vertex_groups['H'].add([1, 6, 10, 66, 70, 87], 0.7, 'REPLACE')
bpy.data.objects['DMaxDist'].vertex_groups['H'].add([2, 9, 26, 34], 0.8, 'REPLACE')
bpy.data.objects['DMaxDist'].vertex_groups['H'].add([19, 25, 30, 33, 36, 67, 78], 0.9, 'REPLACE')
bpy.data.objects['DMaxDist'].vertex_groups['H'].add([3, 32, 53, 61], 1.0, 'REPLACE')
bpy.data.objects['DMaxDist'].modifiers.new(name='DT', type='DATA_TRANSFER')
bpy.data.objects['DMaxDist'].modifiers['DT'].object = bpy.data.objects['S']
bpy.data.objects['DMaxDist'].modifiers['DT'].use_vert_data = True
bpy.data.objects['DMaxDist'].modifiers['DT'].data_types_verts = {'VGROUP_WEIGHTS'}
bpy.data.objects['DMaxDist'].modifiers['DT'].vert_mapping = 'POLYINTERP_NEAREST'
bpy.data.objects['DMaxDist'].modifiers['DT'].use_max_distance = True
bpy.data.objects['DMaxDist'].modifiers['DT'].max_distance = 0.06
bpy.data.objects['DMaxDist'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['DMaxDist'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['DMaxDist'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['DMaxDist'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['DMaxDist'].modifiers['Dz'].strength = 1.0
bpy.data.objects['DMaxDist'].modifiers.new(name='Dx', type='DISPLACE')
bpy.data.objects['DMaxDist'].modifiers['Dx'].vertex_group = 'H'
bpy.data.objects['DMaxDist'].modifiers['Dx'].direction = 'X'
bpy.data.objects['DMaxDist'].modifiers['Dx'].mid_level = 0.0
bpy.data.objects['DMaxDist'].modifiers['Dx'].strength = 0.5
bpy.ops.mesh.primitive_grid_add(x_subdivisions=9, y_subdivisions=8, size=1.9, location=(0.03, -0.02, 0.1), rotation=(0.0, 0.0, 0.13))
bpy.context.object.name = 'DLocal'
bpy.data.objects['DLocal'].vertex_groups.new(name='G')
bpy.data.objects['DLocal'].vertex_groups.new(name='H')
bpy.data.objects['DLocal'].vertex_groups['G'].add([13, 18, 24, 29, 65, 69, 72], 0.1, 'REPLACE')
bpy.data.objects['DLocal'].vertex_groups['G'].add([9, 30, 39, 46, 60, 66, 79], 0.2, 'REPLACE')
bpy.data.objects['DLocal'].vertex_groups['G'].add([25, 34, 51, 57, 87], 0.3, 'REPLACE')
bpy.data.objects['DLocal'].vertex_groups['G'].add([1, 2, 8, 26, 27, 35, 45, 71, 86], 0.4, 'REPLACE')
bpy.data.objects['DLocal'].vertex_groups['G'].add([10, 14, 41, 43, 47, 52, 61, 63, 80, 88], 0.5, 'REPLACE')
bpy.data.objects['DLocal'].vertex_groups['G'].add([11, 42, 49, 54], 0.6, 'REPLACE')
bpy.data.objects['DLocal'].vertex_groups['G'].add([4, 6, 12, 55, 68, 76], 0.7, 'REPLACE')
bpy.data.objects['DLocal'].vertex_groups['G'].add([20, 22, 31, 73, 75, 78, 81], 0.8, 'REPLACE')
bpy.data.objects['DLocal'].vertex_groups['G'].add([17, 23, 38, 40, 48, 53, 83], 0.9, 'REPLACE')
bpy.data.objects['DLocal'].vertex_groups['G'].add([3, 5, 16, 21, 36, 84, 89], 1.0, 'REPLACE')
bpy.data.objects['DLocal'].vertex_groups['H'].add([72], 0.1, 'REPLACE')
bpy.data.objects['DLocal'].vertex_groups['H'].add([24, 33, 52, 55, 71], 0.2, 'REPLACE')
bpy.data.objects['DLocal'].vertex_groups['H'].add([21, 22, 44, 83], 0.3, 'REPLACE')
bpy.data.objects['DLocal'].vertex_groups['H'].add([15, 42, 69, 75, 76], 0.4, 'REPLACE')
bpy.data.objects['DLocal'].vertex_groups['H'].add([1, 10, 12, 14, 23], 0.5, 'REPLACE')
bpy.data.objects['DLocal'].vertex_groups['H'].add([8, 39, 40, 50, 68, 84], 0.6, 'REPLACE')
bpy.data.objects['DLocal'].vertex_groups['H'].add([4, 16, 32, 46, 66, 79], 0.7, 'REPLACE')
bpy.data.objects['DLocal'].vertex_groups['H'].add([63, 65], 0.8, 'REPLACE')
bpy.data.objects['DLocal'].vertex_groups['H'].add([6, 35, 53, 62], 0.9, 'REPLACE')
bpy.data.objects['DLocal'].vertex_groups['H'].add([25, 73, 86, 88], 1.0, 'REPLACE')
bpy.data.objects['DLocal'].modifiers.new(name='DT', type='DATA_TRANSFER')
bpy.data.objects['DLocal'].modifiers['DT'].object = bpy.data.objects['S']
bpy.data.objects['DLocal'].modifiers['DT'].use_vert_data = True
bpy.data.objects['DLocal'].modifiers['DT'].data_types_verts = {'VGROUP_WEIGHTS'}
bpy.data.objects['DLocal'].modifiers['DT'].vert_mapping = 'POLYINTERP_NEAREST'
bpy.data.objects['DLocal'].modifiers['DT'].use_object_transform = False
bpy.data.objects['DLocal'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['DLocal'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['DLocal'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['DLocal'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['DLocal'].modifiers['Dz'].strength = 1.0
bpy.data.objects['DLocal'].modifiers.new(name='Dx', type='DISPLACE')
bpy.data.objects['DLocal'].modifiers['Dx'].vertex_group = 'H'
bpy.data.objects['DLocal'].modifiers['Dx'].direction = 'X'
bpy.data.objects['DLocal'].modifiers['Dx'].mid_level = 0.0
bpy.data.objects['DLocal'].modifiers['Dx'].strength = 0.5
bpy.ops.mesh.primitive_grid_add(x_subdivisions=9, y_subdivisions=8, size=1.9, location=(0.03, -0.02, 0.1), rotation=(0.0, 0.0, 0.13))
bpy.context.object.name = 'DNoH'
bpy.data.objects['DNoH'].vertex_groups.new(name='G')
bpy.data.objects['DNoH'].vertex_groups['G'].add([40, 44, 58], 0.1, 'REPLACE')
bpy.data.objects['DNoH'].vertex_groups['G'].add([0, 36, 47, 78, 80], 0.2, 'REPLACE')
bpy.data.objects['DNoH'].vertex_groups['G'].add([4, 5, 10, 20, 21, 23, 24, 37, 39, 67, 70, 72], 0.3, 'REPLACE')
bpy.data.objects['DNoH'].vertex_groups['G'].add([29, 64, 66, 71, 82, 85], 0.4, 'REPLACE')
bpy.data.objects['DNoH'].vertex_groups['G'].add([3, 30, 31, 32, 45, 55, 61, 68, 77, 84, 89], 0.5, 'REPLACE')
bpy.data.objects['DNoH'].vertex_groups['G'].add([1, 7, 48, 50], 0.6, 'REPLACE')
bpy.data.objects['DNoH'].vertex_groups['G'].add([14, 28, 34, 49, 51, 54], 0.7, 'REPLACE')
bpy.data.objects['DNoH'].vertex_groups['G'].add([2, 22, 33, 41, 43, 69, 75], 0.8, 'REPLACE')
bpy.data.objects['DNoH'].vertex_groups['G'].add([15, 63, 79, 86], 0.9, 'REPLACE')
bpy.data.objects['DNoH'].vertex_groups['G'].add([42, 57, 73, 87], 1.0, 'REPLACE')
bpy.data.objects['DNoH'].modifiers.new(name='DT', type='DATA_TRANSFER')
bpy.data.objects['DNoH'].modifiers['DT'].object = bpy.data.objects['S']
bpy.data.objects['DNoH'].modifiers['DT'].use_vert_data = True
bpy.data.objects['DNoH'].modifiers['DT'].data_types_verts = {'VGROUP_WEIGHTS'}
bpy.data.objects['DNoH'].modifiers['DT'].vert_mapping = 'POLYINTERP_NEAREST'
bpy.data.objects['DNoH'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['DNoH'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['DNoH'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['DNoH'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['DNoH'].modifiers['Dz'].strength = 1.0
