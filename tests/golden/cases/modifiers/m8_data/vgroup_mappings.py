# WP 8.3.13: DATA_TRANSFER of vertex groups (ALL source groups by NAME) with every
# vert_mapping, from a 7x6 grid to a rotated, offset 10x9 grid (TOPOLOGY: the same grid
# in local space). The transferred weights drive DISPLACE along Z (G) and X (H).
# golden: startup=empty compare=evaluated:D0,evaluated:D1,evaluated:D2,evaluated:D3,evaluated:D4,evaluated:D5,evaluated:D6 tier=T2
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=6, y_subdivisions=5, size=2)
bpy.context.object.name = 'S'
bpy.data.objects['S'].vertex_groups.new(name='G')
bpy.data.objects['S'].vertex_groups['G'].add([9, 11], 0.1, 'REPLACE')
bpy.data.objects['S'].vertex_groups['G'].add([20, 21, 35, 36], 0.2, 'REPLACE')
bpy.data.objects['S'].vertex_groups['G'].add([17, 29], 0.3, 'REPLACE')
bpy.data.objects['S'].vertex_groups['G'].add([7, 22, 23, 32, 34], 0.4, 'REPLACE')
bpy.data.objects['S'].vertex_groups['G'].add([1, 26], 0.5, 'REPLACE')
bpy.data.objects['S'].vertex_groups['G'].add([6, 19], 0.6, 'REPLACE')
bpy.data.objects['S'].vertex_groups['G'].add([13, 18, 24, 30, 39], 0.7, 'REPLACE')
bpy.data.objects['S'].vertex_groups['G'].add([0, 31], 0.8, 'REPLACE')
bpy.data.objects['S'].vertex_groups['G'].add([10, 27, 33, 37, 40], 0.9, 'REPLACE')
bpy.data.objects['S'].vertex_groups['G'].add([3, 4, 12, 14, 28, 38], 1.0, 'REPLACE')
bpy.data.objects['S'].vertex_groups.new(name='H')
bpy.data.objects['S'].vertex_groups['H'].add([18], 0.1, 'REPLACE')
bpy.data.objects['S'].vertex_groups['H'].add([14], 0.2, 'REPLACE')
bpy.data.objects['S'].vertex_groups['H'].add([9, 39], 0.3, 'REPLACE')
bpy.data.objects['S'].vertex_groups['H'].add([2, 4, 7, 12, 26, 28, 33, 37], 0.4, 'REPLACE')
bpy.data.objects['S'].vertex_groups['H'].add([1, 6, 8, 15, 25, 31], 0.5, 'REPLACE')
bpy.data.objects['S'].vertex_groups['H'].add([0, 20, 24, 27, 38], 0.6, 'REPLACE')
bpy.data.objects['S'].vertex_groups['H'].add([10, 19, 36], 0.7, 'REPLACE')
bpy.data.objects['S'].vertex_groups['H'].add([13, 17], 0.8, 'REPLACE')
bpy.data.objects['S'].vertex_groups['H'].add([23, 29, 32], 0.9, 'REPLACE')
bpy.data.objects['S'].vertex_groups['H'].add([3, 5, 11, 30, 41], 1.0, 'REPLACE')
bpy.ops.mesh.primitive_grid_add(x_subdivisions=6, y_subdivisions=5, size=2, location=(0.0, 0.0, 0.3))
bpy.context.object.name = 'D0'
bpy.data.objects['D0'].vertex_groups.new(name='G')
bpy.data.objects['D0'].vertex_groups.new(name='H')
bpy.data.objects['D0'].modifiers.new(name='DT', type='DATA_TRANSFER')
bpy.data.objects['D0'].modifiers['DT'].object = bpy.data.objects['S']
bpy.data.objects['D0'].modifiers['DT'].use_vert_data = True
bpy.data.objects['D0'].modifiers['DT'].data_types_verts = {'VGROUP_WEIGHTS'}
bpy.data.objects['D0'].modifiers['DT'].vert_mapping = 'TOPOLOGY'
bpy.data.objects['D0'].modifiers['DT'].use_object_transform = False
bpy.data.objects['D0'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['D0'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['D0'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['D0'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['D0'].modifiers['Dz'].strength = 1.0
bpy.data.objects['D0'].modifiers.new(name='Dx', type='DISPLACE')
bpy.data.objects['D0'].modifiers['Dx'].vertex_group = 'H'
bpy.data.objects['D0'].modifiers['Dx'].direction = 'X'
bpy.data.objects['D0'].modifiers['Dx'].mid_level = 0.0
bpy.data.objects['D0'].modifiers['Dx'].strength = 0.5
bpy.ops.mesh.primitive_grid_add(x_subdivisions=9, y_subdivisions=8, size=1.9, location=(0.03, -0.02, 0.1), rotation=(0.0, 0.0, 0.13))
bpy.context.object.name = 'D1'
bpy.data.objects['D1'].vertex_groups.new(name='G')
bpy.data.objects['D1'].vertex_groups.new(name='H')
bpy.data.objects['D1'].modifiers.new(name='DT', type='DATA_TRANSFER')
bpy.data.objects['D1'].modifiers['DT'].object = bpy.data.objects['S']
bpy.data.objects['D1'].modifiers['DT'].use_vert_data = True
bpy.data.objects['D1'].modifiers['DT'].data_types_verts = {'VGROUP_WEIGHTS'}
bpy.data.objects['D1'].modifiers['DT'].vert_mapping = 'NEAREST'
bpy.data.objects['D1'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['D1'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['D1'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['D1'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['D1'].modifiers['Dz'].strength = 1.0
bpy.data.objects['D1'].modifiers.new(name='Dx', type='DISPLACE')
bpy.data.objects['D1'].modifiers['Dx'].vertex_group = 'H'
bpy.data.objects['D1'].modifiers['Dx'].direction = 'X'
bpy.data.objects['D1'].modifiers['Dx'].mid_level = 0.0
bpy.data.objects['D1'].modifiers['Dx'].strength = 0.5
bpy.ops.mesh.primitive_grid_add(x_subdivisions=9, y_subdivisions=8, size=1.9, location=(0.03, -0.02, 0.1), rotation=(0.0, 0.0, 0.13))
bpy.context.object.name = 'D2'
bpy.data.objects['D2'].vertex_groups.new(name='G')
bpy.data.objects['D2'].vertex_groups.new(name='H')
bpy.data.objects['D2'].modifiers.new(name='DT', type='DATA_TRANSFER')
bpy.data.objects['D2'].modifiers['DT'].object = bpy.data.objects['S']
bpy.data.objects['D2'].modifiers['DT'].use_vert_data = True
bpy.data.objects['D2'].modifiers['DT'].data_types_verts = {'VGROUP_WEIGHTS'}
bpy.data.objects['D2'].modifiers['DT'].vert_mapping = 'EDGE_NEAREST'
bpy.data.objects['D2'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['D2'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['D2'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['D2'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['D2'].modifiers['Dz'].strength = 1.0
bpy.data.objects['D2'].modifiers.new(name='Dx', type='DISPLACE')
bpy.data.objects['D2'].modifiers['Dx'].vertex_group = 'H'
bpy.data.objects['D2'].modifiers['Dx'].direction = 'X'
bpy.data.objects['D2'].modifiers['Dx'].mid_level = 0.0
bpy.data.objects['D2'].modifiers['Dx'].strength = 0.5
bpy.ops.mesh.primitive_grid_add(x_subdivisions=9, y_subdivisions=8, size=1.9, location=(0.03, -0.02, 0.1), rotation=(0.0, 0.0, 0.13))
bpy.context.object.name = 'D3'
bpy.data.objects['D3'].vertex_groups.new(name='G')
bpy.data.objects['D3'].vertex_groups.new(name='H')
bpy.data.objects['D3'].modifiers.new(name='DT', type='DATA_TRANSFER')
bpy.data.objects['D3'].modifiers['DT'].object = bpy.data.objects['S']
bpy.data.objects['D3'].modifiers['DT'].use_vert_data = True
bpy.data.objects['D3'].modifiers['DT'].data_types_verts = {'VGROUP_WEIGHTS'}
bpy.data.objects['D3'].modifiers['DT'].vert_mapping = 'EDGEINTERP_NEAREST'
bpy.data.objects['D3'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['D3'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['D3'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['D3'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['D3'].modifiers['Dz'].strength = 1.0
bpy.data.objects['D3'].modifiers.new(name='Dx', type='DISPLACE')
bpy.data.objects['D3'].modifiers['Dx'].vertex_group = 'H'
bpy.data.objects['D3'].modifiers['Dx'].direction = 'X'
bpy.data.objects['D3'].modifiers['Dx'].mid_level = 0.0
bpy.data.objects['D3'].modifiers['Dx'].strength = 0.5
bpy.ops.mesh.primitive_grid_add(x_subdivisions=9, y_subdivisions=8, size=1.9, location=(0.03, -0.02, 0.1), rotation=(0.0, 0.0, 0.13))
bpy.context.object.name = 'D4'
bpy.data.objects['D4'].vertex_groups.new(name='G')
bpy.data.objects['D4'].vertex_groups.new(name='H')
bpy.data.objects['D4'].modifiers.new(name='DT', type='DATA_TRANSFER')
bpy.data.objects['D4'].modifiers['DT'].object = bpy.data.objects['S']
bpy.data.objects['D4'].modifiers['DT'].use_vert_data = True
bpy.data.objects['D4'].modifiers['DT'].data_types_verts = {'VGROUP_WEIGHTS'}
bpy.data.objects['D4'].modifiers['DT'].vert_mapping = 'POLY_NEAREST'
bpy.data.objects['D4'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['D4'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['D4'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['D4'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['D4'].modifiers['Dz'].strength = 1.0
bpy.data.objects['D4'].modifiers.new(name='Dx', type='DISPLACE')
bpy.data.objects['D4'].modifiers['Dx'].vertex_group = 'H'
bpy.data.objects['D4'].modifiers['Dx'].direction = 'X'
bpy.data.objects['D4'].modifiers['Dx'].mid_level = 0.0
bpy.data.objects['D4'].modifiers['Dx'].strength = 0.5
bpy.ops.mesh.primitive_grid_add(x_subdivisions=9, y_subdivisions=8, size=1.9, location=(0.03, -0.02, 0.1), rotation=(0.0, 0.0, 0.13))
bpy.context.object.name = 'D5'
bpy.data.objects['D5'].vertex_groups.new(name='G')
bpy.data.objects['D5'].vertex_groups.new(name='H')
bpy.data.objects['D5'].modifiers.new(name='DT', type='DATA_TRANSFER')
bpy.data.objects['D5'].modifiers['DT'].object = bpy.data.objects['S']
bpy.data.objects['D5'].modifiers['DT'].use_vert_data = True
bpy.data.objects['D5'].modifiers['DT'].data_types_verts = {'VGROUP_WEIGHTS'}
bpy.data.objects['D5'].modifiers['DT'].vert_mapping = 'POLYINTERP_NEAREST'
bpy.data.objects['D5'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['D5'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['D5'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['D5'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['D5'].modifiers['Dz'].strength = 1.0
bpy.data.objects['D5'].modifiers.new(name='Dx', type='DISPLACE')
bpy.data.objects['D5'].modifiers['Dx'].vertex_group = 'H'
bpy.data.objects['D5'].modifiers['Dx'].direction = 'X'
bpy.data.objects['D5'].modifiers['Dx'].mid_level = 0.0
bpy.data.objects['D5'].modifiers['Dx'].strength = 0.5
bpy.ops.mesh.primitive_grid_add(x_subdivisions=9, y_subdivisions=8, size=1.9, location=(0.03, -0.02, 0.1), rotation=(0.0, 0.0, 0.13))
bpy.context.object.name = 'D6'
bpy.data.objects['D6'].vertex_groups.new(name='G')
bpy.data.objects['D6'].vertex_groups.new(name='H')
bpy.data.objects['D6'].modifiers.new(name='DT', type='DATA_TRANSFER')
bpy.data.objects['D6'].modifiers['DT'].object = bpy.data.objects['S']
bpy.data.objects['D6'].modifiers['DT'].use_vert_data = True
bpy.data.objects['D6'].modifiers['DT'].data_types_verts = {'VGROUP_WEIGHTS'}
bpy.data.objects['D6'].modifiers['DT'].vert_mapping = 'POLYINTERP_VNORPROJ'
bpy.data.objects['D6'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['D6'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['D6'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['D6'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['D6'].modifiers['Dz'].strength = 1.0
bpy.data.objects['D6'].modifiers.new(name='Dx', type='DISPLACE')
bpy.data.objects['D6'].modifiers['Dx'].vertex_group = 'H'
bpy.data.objects['D6'].modifiers['Dx'].direction = 'X'
bpy.data.objects['D6'].modifiers['Dx'].mid_level = 0.0
bpy.data.objects['D6'].modifiers['Dx'].strength = 0.5
