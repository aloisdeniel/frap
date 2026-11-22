# WP 8.3.13: object.data_transfer, each call next to a DATA_TRANSFER modifier with the same
# inputs (O* vs M*, equal results); use_create (groups, attributes, UV maps), the ACTIVE and
# INDEX layer matching, a mix mode, use_reverse_transfer, local space, use_auto_transform,
# islands_precision, use_freeze and a topology mismatch (ERROR report).
# golden: startup=empty compare=objects,evaluated:OVG,evaluated:MVG,evaluated:OEdge,evaluated:MEdge,evaluated:OUV,evaluated:MUV,evaluated:OSmooth,evaluated:MSmooth,evaluated:ONoCreate,evaluated:OActive,evaluated:OIndex,evaluated:OMix,evaluated:ORev,evaluated:OLocal,evaluated:OAuto,evaluated:OIslands,evaluated:OFrozen,evaluated:OTopo tier=T2
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=6, y_subdivisions=5, size=2)
bpy.context.object.name = 'S'
bpy.data.objects['S'].vertex_groups.new(name='G')
bpy.data.objects['S'].vertex_groups['G'].add([9, 30, 34, 41], 0.1, 'REPLACE')
bpy.data.objects['S'].vertex_groups['G'].add([6, 8, 11, 29, 37], 0.2, 'REPLACE')
bpy.data.objects['S'].vertex_groups['G'].add([23, 27], 0.3, 'REPLACE')
bpy.data.objects['S'].vertex_groups['G'].add([1, 3, 4, 19, 20], 0.4, 'REPLACE')
bpy.data.objects['S'].vertex_groups['G'].add([0, 12], 0.5, 'REPLACE')
bpy.data.objects['S'].vertex_groups['G'].add([7, 32], 0.6, 'REPLACE')
bpy.data.objects['S'].vertex_groups['G'].add([2, 10, 14, 15, 24, 28, 31], 0.7, 'REPLACE')
bpy.data.objects['S'].vertex_groups['G'].add([17, 25, 35], 0.8, 'REPLACE')
bpy.data.objects['S'].vertex_groups['G'].add([16, 36, 40], 0.9, 'REPLACE')
bpy.data.objects['S'].vertex_groups['G'].add([13, 33], 1.0, 'REPLACE')
bpy.data.objects['S'].vertex_groups.new(name='H')
bpy.data.objects['S'].vertex_groups['H'].add([0, 7, 12, 27, 29, 31, 39], 0.1, 'REPLACE')
bpy.data.objects['S'].vertex_groups['H'].add([18, 23, 24, 35], 0.2, 'REPLACE')
bpy.data.objects['S'].vertex_groups['H'].add([1, 3, 5, 15, 33], 0.3, 'REPLACE')
bpy.data.objects['S'].vertex_groups['H'].add([2, 6, 16, 21, 41], 0.4, 'REPLACE')
bpy.data.objects['S'].vertex_groups['H'].add([20], 0.5, 'REPLACE')
bpy.data.objects['S'].vertex_groups['H'].add([19, 32, 34, 36], 0.6, 'REPLACE')
bpy.data.objects['S'].vertex_groups['H'].add([9, 14, 17, 40], 0.7, 'REPLACE')
bpy.data.objects['S'].vertex_groups['H'].add([22, 30], 0.8, 'REPLACE')
bpy.data.objects['S'].vertex_groups['H'].add([10, 11, 37], 0.9, 'REPLACE')
bpy.data.objects['S'].vertex_groups['H'].add([13, 26], 1.0, 'REPLACE')
bpy.data.objects['S'].data.attributes.new(name='sharp_edge', type='BOOLEAN', domain='EDGE')
bpy.data.objects['S'].data.attributes.new(name='uv_seam', type='BOOLEAN', domain='EDGE')
bpy.data.objects['S'].data.attributes.new(name='crease_edge', type='FLOAT', domain='EDGE')
bpy.data.objects['S'].data.attributes.new(name='bevel_weight_edge', type='FLOAT', domain='EDGE')
bpy.data.objects['S'].data.attributes.new(name='freestyle_edge', type='BOOLEAN', domain='EDGE')
bpy.data.objects['S'].data.attributes['sharp_edge'].data.foreach_set('value', [True, False, False, False, False, False, False, False, False, True, True, True, True, True, False, False, False, True, False, False, False, False, True, False, True, True, True, False, False, False, False, True, True, False, False, False, False, True, True, False, False, False, False, True, False, True, False, False, False, True, False, False, False, False, False, True, True, True, False, False, False, False, True, True, False, False, False, False, False, False, True])
bpy.data.objects['S'].data.attributes['uv_seam'].data.foreach_set('value', [False, False, True, False, True, False, False, True, False, True, False, True, False, False, False, False, True, True, False, False, True, False, False, True, True, False, True, False, True, True, True, False, True, False, True, False, True, False, False, False, False, False, False, False, True, True, True, True, True, False, False, False, False, True, False, False, False, True, False, False, False, False, False, False, True, False, False, True, False, False, False])
bpy.data.objects['S'].data.attributes['crease_edge'].data.foreach_set('value', [0.826, 0.32, 0.715, 0.399, 0.874, 0.539, 0.985, 0.756, 0.802, 0.074, 0.795, 0.704, 0.934, 0.782, 0.475, 0.588, 0.2, 0.759, 0.924, 0.163, 0.13, 0.645, 0.029, 0.231, 0.899, 0.633, 0.987, 0.112, 0.343, 0.478, 0.498, 0.953, 0.164, 0.702, 0.961, 0.102, 0.271, 0.509, 0.05, 0.117, 0.484, 0.447, 0.603, 0.313, 0.709, 0.13, 0.084, 0.959, 0.55, 0.327, 0.13, 0.616, 0.493, 0.594, 0.628, 0.994, 0.191, 0.944, 0.717, 0.114, 0.21, 0.822, 0.533, 0.149, 0.776, 0.453, 0.004, 0.26, 0.368, 0.648, 0.283])
bpy.data.objects['S'].data.attributes['bevel_weight_edge'].data.foreach_set('value', [0.647, 0.804, 0.518, 0.086, 0.613, 0.132, 0.276, 0.72, 0.472, 0.491, 0.392, 0.46, 0.803, 0.88, 0.582, 0.703, 0.726, 0.524, 0.431, 0.239, 0.954, 0.761, 0.851, 0.389, 0.526, 0.179, 0.369, 0.785, 0.165, 0.459, 0.407, 0.83, 0.06, 0.234, 0.399, 0.156, 0.607, 0.412, 0.567, 0.261, 0.644, 0.329, 0.157, 0.373, 0.528, 0.71, 0.43, 0.997, 0.583, 0.21, 0.851, 0.463, 0.475, 0.708, 0.793, 0.94, 0.102, 0.806, 0.29, 0.581, 0.643, 0.585, 0.473, 0.081, 0.282, 0.025, 0.344, 0.863, 0.115, 0.452, 0.451])
bpy.data.objects['S'].data.attributes['freestyle_edge'].data.foreach_set('value', [False, False, False, True, False, False, True, False, False, False, False, False, True, True, False, False, False, False, True, True, True, True, False, True, True, False, False, False, True, False, False, False, True, True, False, True, False, False, True, False, False, False, False, False, False, False, False, False, False, False, False, True, True, False, True, False, False, True, False, True, False, False, False, False, True, True, False, False, False, True, True])
bpy.data.objects['S'].data.uv_layers['UVMap'].data.foreach_set('uv', [0.628, 0.57, 0.068, 0.933, 0.256, 0.844, 0.75, 0.168, 0.472, 0.208, 0.379, 0.041, 0.549, 0.546, 0.427, 0.502, 0.3, 0.183, 0.631, 0.771, 0.023, 0.764, 0.768, 0.035, 0.097, 0.366, 0.28, 0.899, 0.316, 0.342, 0.106, 0.818, 0.267, 0.586, 0.201, 0.798, 0.052, 0.509, 0.825, 0.468, 0.979, 0.975, 0.602, 0.677, 0.252, 0.26, 0.057, 0.154, 0.155, 0.998, 0.751, 0.655, 0.375, 0.117, 0.378, 0.763, 0.852, 0.001, 0.579, 0.85, 0.496, 0.503, 0.466, 0.479, 0.17, 0.821, 0.815, 0.098, 0.589, 0.866, 0.784, 0.652, 0.178, 0.486, 0.399, 0.426, 0.314, 0.04, 0.094, 0.767, 0.909, 0.371, 0.701, 0.418, 0.066, 0.497, 0.006, 0.661, 0.731, 0.865, 0.511, 0.399, 0.183, 0.749, 0.198, 0.9, 0.585, 0.49, 0.666, 0.403, 0.943, 0.371, 0.792, 0.555, 0.859, 0.81, 0.125, 0.483, 0.814, 0.273, 0.714, 0.583, 0.923, 0.214, 0.174, 0.7, 0.544, 0.871, 0.983, 0.735, 0.055, 0.795, 0.996, 0.253, 0.161, 0.597, 0.57, 0.333, 0.932, 0.885, 0.831, 0.466, 0.211, 0.136, 0.143, 0.599, 0.437, 0.298, 0.085, 0.961, 0.623, 0.345, 0.535, 0.639, 0.449, 0.984, 0.604, 0.266, 0.2, 0.54, 0.449, 0.734, 0.689, 0.75, 0.386, 0.188, 0.841, 0.186, 0.265, 0.131, 0.324, 0.993, 0.448, 0.876, 0.364, 0.6, 0.475, 0.545, 0.098, 0.363, 0.781, 0.771, 0.37, 0.583, 1.0, 0.291, 0.778, 0.623, 0.233, 0.744, 0.257, 0.272, 0.88, 0.831, 0.31, 0.059, 0.365, 0.512, 0.47, 0.443, 0.575, 0.341, 0.37, 0.838, 0.577, 0.494, 0.595, 0.908, 0.403, 0.617, 0.247, 0.587, 0.233, 0.823, 0.151, 0.057, 0.07, 0.047, 0.549, 0.461, 0.851, 0.644, 0.737, 0.202, 0.757, 0.603, 0.092, 0.062, 0.141, 0.881, 0.324, 0.278, 0.595, 0.568, 0.016, 0.49, 0.389, 0.925, 0.411, 0.079, 0.682, 0.319, 0.31, 0.768, 0.354, 0.16, 0.638, 0.303, 0.912, 0.356])
bpy.data.objects['S'].data.attributes['uv_seam'].data.foreach_set('value', [False, False, False, False, False, False, False, False, False, True, False, False, False, False, False, False, False, False, False, True, True, False, True, False, False, False, False, True, False, False, False, False, True, False, False, True, False, False, True, True, False, False, False, False, False, True, True, False, False, False, True, False, False, False, False, False, True, False, False, True, False, False, False, True, False, False, True, False, False, False, False])
bpy.data.objects['S'].data.attributes['sharp_face'].data.foreach_set('value', [False, False, False, True, False, True, True, False, True, True, True, True, False, False, False, False, False, False, True, True, False, False, False, True, False, True, True, False, True, True])
bpy.ops.mesh.primitive_grid_add(x_subdivisions=9, y_subdivisions=8, size=1.9, location=(0.03, -0.02, 0.1), rotation=(0.0, 0.0, 0.13))
bpy.context.object.name = 'OVG'
bpy.ops.mesh.primitive_grid_add(x_subdivisions=9, y_subdivisions=8, size=1.9, location=(0.03, -0.02, 0.1), rotation=(0.0, 0.0, 0.13))
bpy.context.object.name = 'MVG'
bpy.data.objects['MVG'].vertex_groups.new(name='G')
bpy.data.objects['MVG'].vertex_groups.new(name='H')
bpy.data.objects['MVG'].modifiers.new(name='DT', type='DATA_TRANSFER')
bpy.data.objects['MVG'].modifiers['DT'].object = bpy.data.objects['S']
bpy.data.objects['MVG'].modifiers['DT'].use_vert_data = True
bpy.data.objects['MVG'].modifiers['DT'].data_types_verts = {'VGROUP_WEIGHTS'}
bpy.data.objects['MVG'].modifiers['DT'].vert_mapping = 'POLYINTERP_NEAREST'
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['OVG'].select_set(True)
bpy.data.objects['S'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['S']
bpy.ops.object.data_transfer(data_type='VGROUP_WEIGHTS', vert_mapping='POLYINTERP_NEAREST', layers_select_src='ALL', layers_select_dst='NAME')
bpy.data.objects['OVG'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['OVG'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['OVG'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['OVG'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['OVG'].modifiers['Dz'].strength = 1.0
bpy.data.objects['OVG'].modifiers.new(name='Dx', type='DISPLACE')
bpy.data.objects['OVG'].modifiers['Dx'].vertex_group = 'H'
bpy.data.objects['OVG'].modifiers['Dx'].direction = 'X'
bpy.data.objects['OVG'].modifiers['Dx'].mid_level = 0.0
bpy.data.objects['OVG'].modifiers['Dx'].strength = 0.5
bpy.data.objects['MVG'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['MVG'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['MVG'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['MVG'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['MVG'].modifiers['Dz'].strength = 1.0
bpy.data.objects['MVG'].modifiers.new(name='Dx', type='DISPLACE')
bpy.data.objects['MVG'].modifiers['Dx'].vertex_group = 'H'
bpy.data.objects['MVG'].modifiers['Dx'].direction = 'X'
bpy.data.objects['MVG'].modifiers['Dx'].mid_level = 0.0
bpy.data.objects['MVG'].modifiers['Dx'].strength = 0.5
bpy.ops.mesh.primitive_grid_add(x_subdivisions=9, y_subdivisions=8, size=1.9, location=(0.03, -0.02, 0.1), rotation=(0.0, 0.0, 0.13))
bpy.context.object.name = 'OEdge'
bpy.ops.mesh.primitive_grid_add(x_subdivisions=9, y_subdivisions=8, size=1.9, location=(0.03, -0.02, 0.1), rotation=(0.0, 0.0, 0.13))
bpy.context.object.name = 'MEdge'
bpy.data.objects['OEdge'].data.attributes.new(name='crease_edge', type='FLOAT', domain='EDGE')
bpy.data.objects['MEdge'].data.attributes.new(name='crease_edge', type='FLOAT', domain='EDGE')
bpy.data.objects['MEdge'].modifiers.new(name='DT', type='DATA_TRANSFER')
bpy.data.objects['MEdge'].modifiers['DT'].object = bpy.data.objects['S']
bpy.data.objects['MEdge'].modifiers['DT'].use_edge_data = True
bpy.data.objects['MEdge'].modifiers['DT'].data_types_edges = {'CREASE'}
bpy.data.objects['MEdge'].modifiers['DT'].edge_mapping = 'EDGEINTERP_VNORPROJ'
bpy.data.objects['MEdge'].modifiers['DT'].ray_radius = 0.04
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['OEdge'].select_set(True)
bpy.data.objects['S'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['S']
bpy.ops.object.data_transfer(data_type='CREASE', edge_mapping='EDGEINTERP_VNORPROJ', ray_radius=0.04)
bpy.ops.mesh.primitive_grid_add(x_subdivisions=9, y_subdivisions=8, size=1.9, location=(0.03, -0.02, 0.1), rotation=(0.0, 0.0, 0.13))
bpy.context.object.name = 'OUV'
bpy.ops.mesh.primitive_grid_add(x_subdivisions=9, y_subdivisions=8, size=1.9, location=(0.03, -0.02, 0.1), rotation=(0.0, 0.0, 0.13))
bpy.context.object.name = 'MUV'
bpy.data.objects['MUV'].modifiers.new(name='DT', type='DATA_TRANSFER')
bpy.data.objects['MUV'].modifiers['DT'].object = bpy.data.objects['S']
bpy.data.objects['MUV'].modifiers['DT'].use_loop_data = True
bpy.data.objects['MUV'].modifiers['DT'].data_types_loops = {'UV'}
bpy.data.objects['MUV'].modifiers['DT'].loop_mapping = 'POLYINTERP_NEAREST'
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['OUV'].select_set(True)
bpy.data.objects['S'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['S']
bpy.ops.object.data_transfer(data_type='UV', loop_mapping='POLYINTERP_NEAREST', islands_precision=0.0, layers_select_src='ALL', layers_select_dst='NAME')
bpy.ops.mesh.primitive_grid_add(x_subdivisions=9, y_subdivisions=8, size=1.9, location=(0.03, -0.02, 0.1), rotation=(0.0, 0.0, 0.13))
bpy.context.object.name = 'OSmooth'
bpy.ops.mesh.primitive_grid_add(x_subdivisions=9, y_subdivisions=8, size=1.9, location=(0.03, -0.02, 0.1), rotation=(0.0, 0.0, 0.13))
bpy.context.object.name = 'MSmooth'
bpy.data.objects['MSmooth'].modifiers.new(name='DT', type='DATA_TRANSFER')
bpy.data.objects['MSmooth'].modifiers['DT'].object = bpy.data.objects['S']
bpy.data.objects['MSmooth'].modifiers['DT'].use_poly_data = True
bpy.data.objects['MSmooth'].modifiers['DT'].data_types_polys = {'SMOOTH'}
bpy.data.objects['MSmooth'].modifiers['DT'].poly_mapping = 'NEAREST'
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['OSmooth'].select_set(True)
bpy.data.objects['S'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['S']
bpy.ops.object.data_transfer(data_type='SMOOTH', poly_mapping='NEAREST')
bpy.ops.mesh.primitive_grid_add(x_subdivisions=9, y_subdivisions=8, size=1.9, location=(0.03, -0.02, 0.1), rotation=(0.0, 0.0, 0.13))
bpy.context.object.name = 'ONoCreate'
bpy.data.objects['ONoCreate'].vertex_groups.new(name='H')
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['ONoCreate'].select_set(True)
bpy.data.objects['S'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['S']
bpy.ops.object.data_transfer(data_type='VGROUP_WEIGHTS', use_create=False, layers_select_src='ALL', layers_select_dst='NAME')
bpy.data.objects['ONoCreate'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['ONoCreate'].modifiers['Dz'].vertex_group = 'H'
bpy.data.objects['ONoCreate'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['ONoCreate'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['ONoCreate'].modifiers['Dz'].strength = 1.0
bpy.ops.mesh.primitive_grid_add(x_subdivisions=9, y_subdivisions=8, size=1.9, location=(0.03, -0.02, 0.1), rotation=(0.0, 0.0, 0.13))
bpy.context.object.name = 'OActive'
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['OActive'].select_set(True)
bpy.data.objects['S'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['S']
bpy.ops.object.data_transfer(data_type='VGROUP_WEIGHTS')
bpy.data.objects['OActive'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['OActive'].modifiers['Dz'].vertex_group = 'H'
bpy.data.objects['OActive'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['OActive'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['OActive'].modifiers['Dz'].strength = 1.0
bpy.context.view_layer.objects.active = bpy.data.objects['OActive']
bpy.context.object.vertex_groups.active_index
bpy.ops.mesh.primitive_grid_add(x_subdivisions=9, y_subdivisions=8, size=1.9, location=(0.03, -0.02, 0.1), rotation=(0.0, 0.0, 0.13))
bpy.context.object.name = 'OIndex'
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['OIndex'].select_set(True)
bpy.data.objects['S'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['S']
bpy.ops.object.data_transfer(data_type='VGROUP_WEIGHTS', layers_select_src='ALL', layers_select_dst='INDEX')
bpy.data.objects['OIndex'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['OIndex'].modifiers['Dz'].vertex_group = 'Group'
bpy.data.objects['OIndex'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['OIndex'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['OIndex'].modifiers['Dz'].strength = 1.0
bpy.data.objects['OIndex'].modifiers.new(name='Dx', type='DISPLACE')
bpy.data.objects['OIndex'].modifiers['Dx'].vertex_group = 'Group.001'
bpy.data.objects['OIndex'].modifiers['Dx'].direction = 'X'
bpy.data.objects['OIndex'].modifiers['Dx'].mid_level = 0.0
bpy.data.objects['OIndex'].modifiers['Dx'].strength = 0.5
bpy.ops.mesh.primitive_grid_add(x_subdivisions=9, y_subdivisions=8, size=1.9, location=(0.03, -0.02, 0.1), rotation=(0.0, 0.0, 0.13))
bpy.context.object.name = 'OMix'
bpy.data.objects['OMix'].vertex_groups.new(name='G')
bpy.data.objects['OMix'].vertex_groups['G'].add([23, 36, 38, 49, 83], 0.1, 'REPLACE')
bpy.data.objects['OMix'].vertex_groups['G'].add([45, 73], 0.2, 'REPLACE')
bpy.data.objects['OMix'].vertex_groups['G'].add([8, 10, 12, 50, 52, 67, 74], 0.3, 'REPLACE')
bpy.data.objects['OMix'].vertex_groups['G'].add([11, 21, 66, 70], 0.4, 'REPLACE')
bpy.data.objects['OMix'].vertex_groups['G'].add([7, 16, 26, 40, 59, 71, 85, 87], 0.5, 'REPLACE')
bpy.data.objects['OMix'].vertex_groups['G'].add([3, 14, 17, 35, 39, 55, 63, 69, 79, 89], 0.6, 'REPLACE')
bpy.data.objects['OMix'].vertex_groups['G'].add([15, 22, 29, 34, 46, 51, 57], 0.7, 'REPLACE')
bpy.data.objects['OMix'].vertex_groups['G'].add([0, 25, 30, 54, 62, 64, 88], 0.8, 'REPLACE')
bpy.data.objects['OMix'].vertex_groups['G'].add([1, 2, 13, 18, 41, 42, 48, 53, 58, 68, 84, 86], 0.9, 'REPLACE')
bpy.data.objects['OMix'].vertex_groups['G'].add([9, 20, 24, 27, 28, 31, 32, 56, 61, 75, 78, 82], 1.0, 'REPLACE')
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['OMix'].select_set(True)
bpy.data.objects['S'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['S']
bpy.ops.object.data_transfer(data_type='VGROUP_WEIGHTS', layers_select_src='ALL', layers_select_dst='NAME', mix_mode='ADD', mix_factor=0.5)
bpy.data.objects['OMix'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['OMix'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['OMix'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['OMix'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['OMix'].modifiers['Dz'].strength = 1.0
bpy.data.objects['OMix'].modifiers.new(name='Dx', type='DISPLACE')
bpy.data.objects['OMix'].modifiers['Dx'].vertex_group = 'H'
bpy.data.objects['OMix'].modifiers['Dx'].direction = 'X'
bpy.data.objects['OMix'].modifiers['Dx'].mid_level = 0.0
bpy.data.objects['OMix'].modifiers['Dx'].strength = 0.5
bpy.ops.mesh.primitive_grid_add(x_subdivisions=9, y_subdivisions=8, size=1.9, location=(0.03, -0.02, 0.1), rotation=(0.0, 0.0, 0.13))
bpy.context.object.name = 'ORev'
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['S'].select_set(True)
bpy.data.objects['ORev'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['ORev']
bpy.ops.object.data_transfer(use_reverse_transfer=True, data_type='SHARP_EDGE')
bpy.ops.mesh.primitive_grid_add(x_subdivisions=9, y_subdivisions=8, size=1.9, location=(0.03, -0.02, 0.1), rotation=(0.0, 0.0, 0.13))
bpy.context.object.name = 'OLocal'
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['OLocal'].select_set(True)
bpy.data.objects['S'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['S']
bpy.ops.object.data_transfer(data_type='SEAM', use_object_transform=False)
bpy.ops.mesh.primitive_grid_add(x_subdivisions=6, y_subdivisions=5, size=2, location=(3.0, 1.0, 0.5), rotation=(0.0, 0.0, 0.3))
bpy.context.object.name = 'OAuto'
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['OAuto'].select_set(True)
bpy.data.objects['S'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['S']
bpy.ops.object.data_transfer(data_type='BEVEL_WEIGHT_EDGE', use_auto_transform=True)
bpy.ops.mesh.primitive_grid_add(x_subdivisions=9, y_subdivisions=8, size=1.9, location=(0.03, -0.02, 0.1), rotation=(0.0, 0.0, 0.13))
bpy.context.object.name = 'OIslands'
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['OIslands'].select_set(True)
bpy.data.objects['S'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['S']
bpy.ops.object.data_transfer(data_type='UV', loop_mapping='POLYINTERP_LNORPROJ', islands_precision=0.5, layers_select_src='ALL', layers_select_dst='NAME')
bpy.ops.mesh.primitive_grid_add(x_subdivisions=9, y_subdivisions=8, size=1.9, location=(0.03, -0.02, 0.1), rotation=(0.0, 0.0, 0.13))
bpy.context.object.name = 'OFrozen'
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['OFrozen'].select_set(True)
bpy.data.objects['S'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['S']
bpy.ops.object.data_transfer(data_type='SHARP_EDGE', use_freeze=True)
bpy.ops.mesh.primitive_grid_add(x_subdivisions=9, y_subdivisions=8, size=1.9, location=(0.03, -0.02, 0.1), rotation=(0.0, 0.0, 0.13))
bpy.context.object.name = 'OTopo'
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['OTopo'].select_set(True)
bpy.data.objects['S'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['S']
bpy.ops.object.data_transfer(data_type='SHARP_EDGE', edge_mapping='TOPOLOGY')
