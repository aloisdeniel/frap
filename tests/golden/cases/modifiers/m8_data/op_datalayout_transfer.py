# WP 8.3.13: object.datalayout_transfer: the layers of each data type created on the
# selected meshes (vertex groups, UV maps, colour attributes, edge bevel weights and sharp
# marks), use_delete removing the destination's extra groups and UV maps, INDEX matching,
# and the DATA_TRANSFER modifier's 'Generate Data Layers' (modifier='DT'). Blender tags
# each selected object in order, which drops the source's evaluated mesh: objects after
# the source in the selection order ('Late') get nothing.
# golden: startup=empty compare=objects,evaluated:A,evaluated:B,evaluated:C,evaluated:Late,evaluated:M tier=T2
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=9, y_subdivisions=8, size=1.9, location=(0.03, -0.02, 0.1), rotation=(0.0, 0.0, 0.13))
bpy.context.object.name = 'A'
bpy.ops.mesh.primitive_grid_add(x_subdivisions=9, y_subdivisions=8, size=1.9, location=(0.03, -0.02, 0.1), rotation=(0.0, 0.0, 0.13))
bpy.context.object.name = 'B'
bpy.data.objects['B'].vertex_groups.new(name='Extra')
bpy.data.objects['B'].vertex_groups.new(name='G')
bpy.data.objects['B'].data.uv_layers.new(name='Other')
bpy.ops.mesh.primitive_grid_add(x_subdivisions=9, y_subdivisions=8, size=1.9, location=(0.03, -0.02, 0.1), rotation=(0.0, 0.0, 0.13))
bpy.context.object.name = 'C'
bpy.ops.mesh.primitive_grid_add(x_subdivisions=6, y_subdivisions=5, size=2)
bpy.context.object.name = 'S'
bpy.data.objects['S'].vertex_groups.new(name='G')
bpy.data.objects['S'].vertex_groups['G'].add([11, 12, 24], 0.1, 'REPLACE')
bpy.data.objects['S'].vertex_groups['G'].add([9, 13, 14, 31, 40], 0.2, 'REPLACE')
bpy.data.objects['S'].vertex_groups['G'].add([2, 26, 33], 0.3, 'REPLACE')
bpy.data.objects['S'].vertex_groups['G'].add([6, 21, 29, 35], 0.4, 'REPLACE')
bpy.data.objects['S'].vertex_groups['G'].add([19, 20, 30], 0.5, 'REPLACE')
bpy.data.objects['S'].vertex_groups['G'].add([5, 8], 0.6, 'REPLACE')
bpy.data.objects['S'].vertex_groups['G'].add([1, 3, 7, 17], 0.7, 'REPLACE')
bpy.data.objects['S'].vertex_groups['G'].add([10, 25, 36, 38], 0.8, 'REPLACE')
bpy.data.objects['S'].vertex_groups['G'].add([4, 32, 39, 41], 0.9, 'REPLACE')
bpy.data.objects['S'].vertex_groups['G'].add([16, 27, 28, 37], 1.0, 'REPLACE')
bpy.data.objects['S'].vertex_groups.new(name='H')
bpy.data.objects['S'].vertex_groups['H'].add([8], 0.1, 'REPLACE')
bpy.data.objects['S'].vertex_groups['H'].add([9, 10, 17, 31, 37, 39], 0.2, 'REPLACE')
bpy.data.objects['S'].vertex_groups['H'].add([7, 24, 25], 0.3, 'REPLACE')
bpy.data.objects['S'].vertex_groups['H'].add([6, 35, 40], 0.4, 'REPLACE')
bpy.data.objects['S'].vertex_groups['H'].add([14, 19], 0.5, 'REPLACE')
bpy.data.objects['S'].vertex_groups['H'].add([0, 3, 20, 23], 0.6, 'REPLACE')
bpy.data.objects['S'].vertex_groups['H'].add([15, 30, 34], 0.7, 'REPLACE')
bpy.data.objects['S'].vertex_groups['H'].add([4, 18, 38], 0.8, 'REPLACE')
bpy.data.objects['S'].vertex_groups['H'].add([1, 12, 16, 22, 33], 0.9, 'REPLACE')
bpy.data.objects['S'].vertex_groups['H'].add([13, 27, 32, 41], 1.0, 'REPLACE')
bpy.data.objects['S'].data.uv_layers.new(name='Second')
bpy.data.objects['S'].data.attributes.new(name='Col', type='FLOAT_COLOR', domain='POINT')
bpy.data.objects['S'].data.attributes.new(name='BCol', type='BYTE_COLOR', domain='CORNER')
bpy.data.objects['S'].data.attributes.new(name='bevel_weight_edge', type='FLOAT', domain='EDGE')
bpy.ops.mesh.primitive_grid_add(x_subdivisions=9, y_subdivisions=8, size=1.9, location=(0.03, -0.02, 0.1), rotation=(0.0, 0.0, 0.13))
bpy.context.object.name = 'Late'
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['A'].select_set(True)
bpy.data.objects['S'].select_set(True)
bpy.data.objects['Late'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['S']
bpy.ops.object.datalayout_transfer(data_type='VGROUP_WEIGHTS', layers_select_src='ALL', layers_select_dst='NAME')
bpy.ops.object.datalayout_transfer(data_type='UV', layers_select_src='ALL', layers_select_dst='NAME')
bpy.ops.object.datalayout_transfer(data_type='COLOR_VERTEX', layers_select_src='ALL', layers_select_dst='NAME')
bpy.ops.object.datalayout_transfer(data_type='COLOR_CORNER', layers_select_src='ALL', layers_select_dst='NAME')
bpy.ops.object.datalayout_transfer(data_type='BEVEL_WEIGHT_EDGE')
bpy.ops.object.datalayout_transfer(data_type='SHARP_EDGE')
bpy.data.objects['A'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['A'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['A'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['A'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['A'].modifiers['Dz'].strength = 1.0
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['B'].select_set(True)
bpy.data.objects['S'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['S']
bpy.ops.object.datalayout_transfer(data_type='VGROUP_WEIGHTS', use_delete=True, layers_select_src='ALL', layers_select_dst='NAME')
bpy.ops.object.datalayout_transfer(data_type='UV', use_delete=True, layers_select_src='ALL', layers_select_dst='NAME')
bpy.data.objects['B'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['B'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['B'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['B'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['B'].modifiers['Dz'].strength = 1.0
bpy.data.objects['B'].modifiers.new(name='Dx', type='DISPLACE')
bpy.data.objects['B'].modifiers['Dx'].vertex_group = 'H'
bpy.data.objects['B'].modifiers['Dx'].direction = 'X'
bpy.data.objects['B'].modifiers['Dx'].mid_level = 0.0
bpy.data.objects['B'].modifiers['Dx'].strength = 1.0
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['C'].select_set(True)
bpy.data.objects['S'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['S']
bpy.ops.object.datalayout_transfer(data_type='VGROUP_WEIGHTS', layers_select_src='ALL', layers_select_dst='INDEX')
bpy.data.objects['C'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['C'].modifiers['Dz'].vertex_group = 'Group'
bpy.data.objects['C'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['C'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['C'].modifiers['Dz'].strength = 1.0
bpy.ops.mesh.primitive_grid_add(x_subdivisions=9, y_subdivisions=8, size=1.9, location=(0.03, -0.02, 0.1), rotation=(0.0, 0.0, 0.13))
bpy.context.object.name = 'M'
bpy.data.objects['M'].modifiers.new(name='DT', type='DATA_TRANSFER')
bpy.data.objects['M'].modifiers['DT'].object = bpy.data.objects['S']
bpy.data.objects['M'].modifiers['DT'].use_vert_data = True
bpy.data.objects['M'].modifiers['DT'].data_types_verts = {'VGROUP_WEIGHTS'}
bpy.data.objects['M'].modifiers['DT'].use_loop_data = True
bpy.data.objects['M'].modifiers['DT'].data_types_loops = {'UV'}
bpy.data.objects['M'].modifiers['DT'].loop_mapping = 'POLYINTERP_NEAREST'
bpy.data.objects['M'].modifiers['DT'].use_edge_data = True
bpy.data.objects['M'].modifiers['DT'].data_types_edges = {'BEVEL_WEIGHT_EDGE'}
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['M'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['M']
bpy.ops.object.datalayout_transfer(modifier='DT')
bpy.data.objects['M'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['M'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['M'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['M'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['M'].modifiers['Dz'].strength = 1.0
