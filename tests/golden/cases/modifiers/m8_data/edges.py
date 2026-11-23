# WP 8.3.13: DATA_TRANSFER of the edge data (sharp, seam, crease, bevel weight, freestyle
# mark) with every edge_mapping (EDGEINTERP_VNORPROJ with and without a ray radius) and a
# max distance that leaves far edges untouched.
# golden: startup=empty compare=evaluated:DTopo,evaluated:DVertNearest,evaluated:DNearest,evaluated:DPolyNearest,evaluated:DVnorproj,evaluated:DVnorprojRay,evaluated:DMaxDist tier=T2
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=6, y_subdivisions=5, size=2)
bpy.context.object.name = 'S'
bpy.data.objects['S'].data.attributes.new(name='sharp_edge', type='BOOLEAN', domain='EDGE')
bpy.data.objects['S'].data.attributes.new(name='uv_seam', type='BOOLEAN', domain='EDGE')
bpy.data.objects['S'].data.attributes.new(name='crease_edge', type='FLOAT', domain='EDGE')
bpy.data.objects['S'].data.attributes.new(name='bevel_weight_edge', type='FLOAT', domain='EDGE')
bpy.data.objects['S'].data.attributes.new(name='freestyle_edge', type='BOOLEAN', domain='EDGE')
bpy.data.objects['S'].data.attributes['sharp_edge'].data.foreach_set('value', [False, False, False, False, False, False, False, False, True, False, False, False, True, False, True, False, False, False, True, False, True, True, False, False, False, False, False, False, False, False, True, True, True, False, True, False, False, True, False, True, True, False, False, False, False, False, False, False, False, True, True, True, False, True, True, True, False, False, False, False, False, False, False, False, True, True, True, True, False, True, False])
bpy.data.objects['S'].data.attributes['uv_seam'].data.foreach_set('value', [True, True, True, True, True, False, True, False, False, False, True, False, False, False, False, False, True, True, True, False, False, False, False, False, False, False, False, False, False, True, True, False, False, False, False, False, False, False, False, False, True, True, False, True, True, False, True, True, True, True, False, True, False, True, False, False, False, True, False, True, False, True, True, False, True, False, True, False, False, False, False])
bpy.data.objects['S'].data.attributes['crease_edge'].data.foreach_set('value', [0.232, 0.532, 0.917, 0.577, 0.326, 0.56, 0.643, 0.553, 0.663, 0.458, 0.318, 0.432, 0.426, 0.846, 0.94, 0.385, 0.084, 0.146, 0.763, 0.683, 0.056, 0.285, 0.637, 0.593, 0.898, 0.607, 0.108, 0.467, 0.339, 0.042, 0.838, 0.809, 0.163, 0.249, 0.69, 0.228, 0.205, 0.709, 0.364, 0.041, 0.715, 0.9, 0.565, 0.364, 0.099, 0.127, 0.719, 0.921, 0.979, 0.241, 0.674, 0.211, 0.632, 0.717, 0.203, 0.89, 0.868, 0.0, 0.569, 0.056, 0.264, 0.819, 0.949, 0.515, 0.0, 0.083, 0.384, 0.319, 0.066, 0.7, 0.409])
bpy.data.objects['S'].data.attributes['bevel_weight_edge'].data.foreach_set('value', [0.173, 0.387, 0.133, 0.916, 0.808, 0.91, 0.728, 0.254, 0.208, 0.924, 0.428, 0.029, 0.041, 0.214, 0.748, 0.184, 0.017, 0.307, 0.216, 0.072, 0.346, 0.295, 0.995, 0.074, 0.934, 0.274, 0.604, 0.792, 0.423, 0.618, 0.96, 0.678, 0.021, 0.478, 0.533, 0.247, 0.778, 0.27, 0.035, 0.108, 0.616, 0.819, 0.549, 0.194, 0.761, 0.265, 0.909, 0.446, 0.066, 0.031, 0.65, 0.967, 0.336, 0.404, 0.466, 0.919, 0.9, 0.473, 0.891, 0.986, 0.261, 0.108, 0.443, 0.714, 0.791, 0.575, 0.139, 0.751, 0.125, 0.983, 0.696])
bpy.data.objects['S'].data.attributes['freestyle_edge'].data.foreach_set('value', [True, False, False, False, False, False, False, False, True, False, False, True, False, False, False, True, False, True, False, False, False, True, False, False, False, False, True, False, True, True, True, True, True, False, False, True, False, True, False, False, False, True, False, False, False, False, False, True, True, False, False, True, True, False, False, False, False, False, False, True, True, True, False, True, False, False, True, False, False, False, False])
bpy.ops.mesh.primitive_grid_add(x_subdivisions=6, y_subdivisions=5, size=2, location=(0.0, 0.0, 0.3))
bpy.context.object.name = 'DTopo'
bpy.data.objects['DTopo'].data.attributes.new(name='sharp_edge', type='BOOLEAN', domain='EDGE')
bpy.data.objects['DTopo'].data.attributes.new(name='uv_seam', type='BOOLEAN', domain='EDGE')
bpy.data.objects['DTopo'].data.attributes.new(name='crease_edge', type='FLOAT', domain='EDGE')
bpy.data.objects['DTopo'].data.attributes.new(name='bevel_weight_edge', type='FLOAT', domain='EDGE')
bpy.data.objects['DTopo'].data.attributes.new(name='freestyle_edge', type='BOOLEAN', domain='EDGE')
bpy.data.objects['DTopo'].modifiers.new(name='DT', type='DATA_TRANSFER')
bpy.data.objects['DTopo'].modifiers['DT'].object = bpy.data.objects['S']
bpy.data.objects['DTopo'].modifiers['DT'].use_edge_data = True
bpy.data.objects['DTopo'].modifiers['DT'].data_types_edges = {'SHARP_EDGE', 'SEAM', 'BEVEL_WEIGHT_EDGE', 'CREASE', 'FREESTYLE_EDGE'}
bpy.data.objects['DTopo'].modifiers['DT'].edge_mapping = 'TOPOLOGY'
bpy.data.objects['DTopo'].modifiers['DT'].use_object_transform = False
bpy.ops.mesh.primitive_grid_add(x_subdivisions=9, y_subdivisions=8, size=1.9, location=(0.03, -0.02, 0.1), rotation=(0.0, 0.0, 0.13))
bpy.context.object.name = 'DVertNearest'
bpy.data.objects['DVertNearest'].data.attributes.new(name='sharp_edge', type='BOOLEAN', domain='EDGE')
bpy.data.objects['DVertNearest'].data.attributes.new(name='uv_seam', type='BOOLEAN', domain='EDGE')
bpy.data.objects['DVertNearest'].data.attributes.new(name='crease_edge', type='FLOAT', domain='EDGE')
bpy.data.objects['DVertNearest'].data.attributes.new(name='bevel_weight_edge', type='FLOAT', domain='EDGE')
bpy.data.objects['DVertNearest'].data.attributes.new(name='freestyle_edge', type='BOOLEAN', domain='EDGE')
bpy.data.objects['DVertNearest'].modifiers.new(name='DT', type='DATA_TRANSFER')
bpy.data.objects['DVertNearest'].modifiers['DT'].object = bpy.data.objects['S']
bpy.data.objects['DVertNearest'].modifiers['DT'].use_edge_data = True
bpy.data.objects['DVertNearest'].modifiers['DT'].data_types_edges = {'SHARP_EDGE', 'SEAM', 'BEVEL_WEIGHT_EDGE', 'CREASE', 'FREESTYLE_EDGE'}
bpy.data.objects['DVertNearest'].modifiers['DT'].edge_mapping = 'VERT_NEAREST'
bpy.ops.mesh.primitive_grid_add(x_subdivisions=9, y_subdivisions=8, size=1.9, location=(0.03, -0.02, 0.1), rotation=(0.0, 0.0, 0.13))
bpy.context.object.name = 'DNearest'
bpy.data.objects['DNearest'].data.attributes.new(name='sharp_edge', type='BOOLEAN', domain='EDGE')
bpy.data.objects['DNearest'].data.attributes.new(name='uv_seam', type='BOOLEAN', domain='EDGE')
bpy.data.objects['DNearest'].data.attributes.new(name='crease_edge', type='FLOAT', domain='EDGE')
bpy.data.objects['DNearest'].data.attributes.new(name='bevel_weight_edge', type='FLOAT', domain='EDGE')
bpy.data.objects['DNearest'].data.attributes.new(name='freestyle_edge', type='BOOLEAN', domain='EDGE')
bpy.data.objects['DNearest'].modifiers.new(name='DT', type='DATA_TRANSFER')
bpy.data.objects['DNearest'].modifiers['DT'].object = bpy.data.objects['S']
bpy.data.objects['DNearest'].modifiers['DT'].use_edge_data = True
bpy.data.objects['DNearest'].modifiers['DT'].data_types_edges = {'SHARP_EDGE', 'SEAM', 'BEVEL_WEIGHT_EDGE', 'CREASE', 'FREESTYLE_EDGE'}
bpy.data.objects['DNearest'].modifiers['DT'].edge_mapping = 'NEAREST'
bpy.ops.mesh.primitive_grid_add(x_subdivisions=9, y_subdivisions=8, size=1.9, location=(0.03, -0.02, 0.1), rotation=(0.0, 0.0, 0.13))
bpy.context.object.name = 'DPolyNearest'
bpy.data.objects['DPolyNearest'].data.attributes.new(name='sharp_edge', type='BOOLEAN', domain='EDGE')
bpy.data.objects['DPolyNearest'].data.attributes.new(name='uv_seam', type='BOOLEAN', domain='EDGE')
bpy.data.objects['DPolyNearest'].data.attributes.new(name='crease_edge', type='FLOAT', domain='EDGE')
bpy.data.objects['DPolyNearest'].data.attributes.new(name='bevel_weight_edge', type='FLOAT', domain='EDGE')
bpy.data.objects['DPolyNearest'].data.attributes.new(name='freestyle_edge', type='BOOLEAN', domain='EDGE')
bpy.data.objects['DPolyNearest'].modifiers.new(name='DT', type='DATA_TRANSFER')
bpy.data.objects['DPolyNearest'].modifiers['DT'].object = bpy.data.objects['S']
bpy.data.objects['DPolyNearest'].modifiers['DT'].use_edge_data = True
bpy.data.objects['DPolyNearest'].modifiers['DT'].data_types_edges = {'SHARP_EDGE', 'SEAM', 'BEVEL_WEIGHT_EDGE', 'CREASE', 'FREESTYLE_EDGE'}
bpy.data.objects['DPolyNearest'].modifiers['DT'].edge_mapping = 'POLY_NEAREST'
bpy.ops.mesh.primitive_grid_add(x_subdivisions=9, y_subdivisions=8, size=1.9, location=(0.03, -0.02, 0.1), rotation=(0.0, 0.0, 0.13))
bpy.context.object.name = 'DVnorproj'
bpy.data.objects['DVnorproj'].data.attributes.new(name='sharp_edge', type='BOOLEAN', domain='EDGE')
bpy.data.objects['DVnorproj'].data.attributes.new(name='uv_seam', type='BOOLEAN', domain='EDGE')
bpy.data.objects['DVnorproj'].data.attributes.new(name='crease_edge', type='FLOAT', domain='EDGE')
bpy.data.objects['DVnorproj'].data.attributes.new(name='bevel_weight_edge', type='FLOAT', domain='EDGE')
bpy.data.objects['DVnorproj'].data.attributes.new(name='freestyle_edge', type='BOOLEAN', domain='EDGE')
bpy.data.objects['DVnorproj'].modifiers.new(name='DT', type='DATA_TRANSFER')
bpy.data.objects['DVnorproj'].modifiers['DT'].object = bpy.data.objects['S']
bpy.data.objects['DVnorproj'].modifiers['DT'].use_edge_data = True
bpy.data.objects['DVnorproj'].modifiers['DT'].data_types_edges = {'SHARP_EDGE', 'SEAM', 'BEVEL_WEIGHT_EDGE', 'CREASE', 'FREESTYLE_EDGE'}
bpy.data.objects['DVnorproj'].modifiers['DT'].edge_mapping = 'EDGEINTERP_VNORPROJ'
bpy.data.objects['DVnorproj'].modifiers['DT'].ray_radius = 0.05
bpy.ops.mesh.primitive_grid_add(x_subdivisions=9, y_subdivisions=8, size=1.9, location=(0.03, -0.02, 0.1), rotation=(0.0, 0.0, 0.13))
bpy.context.object.name = 'DVnorprojRay'
bpy.data.objects['DVnorprojRay'].data.attributes.new(name='sharp_edge', type='BOOLEAN', domain='EDGE')
bpy.data.objects['DVnorprojRay'].data.attributes.new(name='uv_seam', type='BOOLEAN', domain='EDGE')
bpy.data.objects['DVnorprojRay'].data.attributes.new(name='crease_edge', type='FLOAT', domain='EDGE')
bpy.data.objects['DVnorprojRay'].data.attributes.new(name='bevel_weight_edge', type='FLOAT', domain='EDGE')
bpy.data.objects['DVnorprojRay'].data.attributes.new(name='freestyle_edge', type='BOOLEAN', domain='EDGE')
bpy.data.objects['DVnorprojRay'].modifiers.new(name='DT', type='DATA_TRANSFER')
bpy.data.objects['DVnorprojRay'].modifiers['DT'].object = bpy.data.objects['S']
bpy.data.objects['DVnorprojRay'].modifiers['DT'].use_edge_data = True
bpy.data.objects['DVnorprojRay'].modifiers['DT'].data_types_edges = {'SHARP_EDGE', 'SEAM', 'BEVEL_WEIGHT_EDGE', 'CREASE', 'FREESTYLE_EDGE'}
bpy.data.objects['DVnorprojRay'].modifiers['DT'].edge_mapping = 'EDGEINTERP_VNORPROJ'
bpy.data.objects['DVnorprojRay'].modifiers['DT'].ray_radius = 0.0
bpy.ops.mesh.primitive_grid_add(x_subdivisions=9, y_subdivisions=8, size=1.9, location=(0.03, -0.02, 0.1), rotation=(0.0, 0.0, 0.13))
bpy.context.object.name = 'DMaxDist'
bpy.data.objects['DMaxDist'].data.attributes.new(name='sharp_edge', type='BOOLEAN', domain='EDGE')
bpy.data.objects['DMaxDist'].data.attributes.new(name='uv_seam', type='BOOLEAN', domain='EDGE')
bpy.data.objects['DMaxDist'].data.attributes.new(name='crease_edge', type='FLOAT', domain='EDGE')
bpy.data.objects['DMaxDist'].data.attributes.new(name='bevel_weight_edge', type='FLOAT', domain='EDGE')
bpy.data.objects['DMaxDist'].data.attributes.new(name='freestyle_edge', type='BOOLEAN', domain='EDGE')
bpy.data.objects['DMaxDist'].data.attributes['sharp_edge'].data.foreach_set('value', [True, False, False, False, False, True, False, False, False, False, False, False, True, False, False, True, False, False, False, False, False, True, False, False, True, True, False, True, False, False, False, False, True, False, True, True, False, True, True, True, False, False, False, True, True, True, True, False, False, False, True, True, True, True, True, True, True, True, False, False, False, False, True, False, True, False, True, False, True, True, False, True, True, False, False, False, True, True, True, False, False, False, False, True, False, False, True, False, False, True, True, True, True, False, False, True, False, False, False, True, False, False, False, False, True, False, True, True, False, False, False, True, False, True, False, False, True, False, False, False, False, False, False, False, False, False, True, False, False, True, False, False, False, True, False, False, False, False, False, True, False, False, False, True, False, False, False, True, False, False, False, False, True, True, True, False, False, False, False, False, False])
bpy.data.objects['DMaxDist'].data.attributes['uv_seam'].data.foreach_set('value', [True, True, False, True, True, False, False, False, False, False, True, False, False, False, False, False, False, False, False, False, False, False, False, False, True, True, False, True, True, False, False, True, False, False, False, True, False, False, False, True, True, False, False, True, False, False, False, False, False, False, False, False, False, False, True, True, False, True, False, False, False, False, False, False, False, False, False, False, False, False, True, False, False, False, False, False, True, True, False, False, False, True, False, True, False, False, False, False, True, True, False, True, True, True, False, True, True, False, False, False, True, False, True, True, False, False, True, False, False, True, False, True, False, False, False, True, True, True, False, True, False, True, True, False, False, False, True, False, False, False, False, False, False, False, False, True, False, False, False, False, True, True, False, True, False, True, False, False, True, True, False, False, True, True, False, False, False, True, True, False, False])
bpy.data.objects['DMaxDist'].data.attributes['crease_edge'].data.foreach_set('value', [0.943, 0.094, 0.072, 0.159, 0.54, 0.659, 0.814, 0.512, 0.927, 0.619, 0.381, 0.651, 0.345, 0.849, 0.608, 0.905, 0.322, 0.756, 0.512, 0.683, 0.943, 0.85, 0.519, 0.561, 0.077, 0.434, 0.009, 0.063, 0.864, 0.311, 0.185, 0.848, 0.814, 0.614, 0.008, 0.628, 0.391, 0.666, 0.496, 0.29, 0.665, 0.839, 0.432, 0.603, 0.823, 0.702, 0.553, 0.832, 0.981, 0.298, 0.859, 0.568, 0.922, 0.107, 0.943, 0.423, 0.009, 0.254, 0.829, 0.464, 0.452, 0.537, 0.642, 0.325, 0.831, 0.976, 0.231, 0.588, 0.462, 0.311, 0.462, 0.172, 0.81, 0.831, 0.542, 0.504, 0.918, 0.76, 0.091, 0.678, 0.792, 0.776, 0.854, 0.868, 0.186, 0.113, 0.062, 0.756, 0.199, 0.063, 0.744, 0.16, 0.188, 0.781, 0.721, 0.832, 0.119, 0.383, 0.467, 0.255, 0.558, 0.264, 0.763, 0.686, 0.406, 0.514, 0.555, 0.87, 0.017, 0.134, 0.512, 0.621, 0.955, 0.815, 0.132, 0.403, 0.371, 0.789, 0.936, 0.727, 0.002, 0.541, 0.164, 0.462, 0.562, 0.366, 0.503, 0.777, 0.459, 0.145, 0.04, 0.909, 0.241, 0.833, 0.445, 0.865, 0.462, 0.126, 0.271, 0.53, 0.926, 0.411, 0.907, 0.481, 0.656, 0.599, 0.304, 0.371, 0.078, 0.584, 0.733, 0.509, 0.501, 0.544, 0.46, 0.993, 0.639, 0.557, 0.587, 0.836, 0.366])
bpy.data.objects['DMaxDist'].data.attributes['bevel_weight_edge'].data.foreach_set('value', [0.506, 0.712, 0.694, 0.097, 0.177, 0.034, 0.263, 0.753, 0.671, 0.526, 0.889, 0.534, 0.874, 0.018, 0.678, 0.91, 0.074, 0.38, 0.242, 0.061, 0.749, 0.687, 0.217, 0.91, 0.495, 0.756, 0.272, 0.054, 0.985, 0.066, 0.678, 0.125, 0.878, 0.679, 0.914, 0.588, 0.595, 0.459, 0.824, 0.659, 0.537, 0.898, 0.833, 0.263, 0.438, 0.524, 0.654, 0.856, 0.544, 0.764, 0.828, 0.82, 0.655, 0.124, 0.383, 0.206, 0.543, 0.4, 0.644, 0.917, 0.684, 0.002, 0.573, 0.898, 0.144, 0.646, 0.91, 0.624, 0.66, 0.293, 0.779, 0.957, 0.586, 0.747, 0.204, 0.092, 0.058, 0.104, 0.204, 0.193, 0.227, 0.509, 0.119, 0.767, 0.858, 0.518, 0.486, 0.812, 0.464, 0.64, 0.532, 0.193, 0.724, 0.454, 0.603, 0.558, 0.286, 0.007, 0.752, 0.726, 0.135, 0.114, 0.131, 0.474, 0.497, 0.533, 0.819, 0.217, 0.435, 0.289, 0.559, 0.946, 0.499, 0.096, 0.851, 0.196, 0.454, 0.032, 0.42, 0.909, 0.264, 0.279, 0.583, 0.533, 0.499, 0.582, 0.527, 0.474, 0.045, 0.793, 0.128, 0.19, 0.762, 0.679, 0.816, 0.577, 0.513, 0.548, 0.231, 0.999, 0.673, 0.746, 0.075, 0.645, 0.683, 0.144, 0.438, 0.628, 0.071, 0.714, 0.855, 0.948, 0.168, 0.334, 0.849, 0.491, 0.073, 0.709, 0.405, 0.552, 0.506])
bpy.data.objects['DMaxDist'].data.attributes['freestyle_edge'].data.foreach_set('value', [False, False, True, False, True, False, False, True, True, True, False, False, True, True, False, True, True, False, False, True, True, True, False, True, True, False, False, False, False, False, False, False, False, True, True, False, True, True, True, True, False, False, False, False, False, False, False, False, False, True, False, False, True, True, False, False, False, True, False, False, True, False, False, False, False, False, False, False, True, False, True, True, True, False, True, False, False, False, False, False, False, False, False, False, True, True, True, False, False, True, False, False, False, True, False, True, True, False, False, False, False, False, True, False, False, True, False, False, False, False, False, True, False, False, False, False, True, False, False, False, False, False, False, False, True, False, False, True, False, False, False, False, False, True, False, False, True, True, True, True, True, False, True, True, True, False, False, False, True, False, False, True, False, False, True, True, False, False, True, False, False])
bpy.data.objects['DMaxDist'].modifiers.new(name='DT', type='DATA_TRANSFER')
bpy.data.objects['DMaxDist'].modifiers['DT'].object = bpy.data.objects['S']
bpy.data.objects['DMaxDist'].modifiers['DT'].use_edge_data = True
bpy.data.objects['DMaxDist'].modifiers['DT'].data_types_edges = {'SHARP_EDGE', 'SEAM', 'BEVEL_WEIGHT_EDGE', 'CREASE', 'FREESTYLE_EDGE'}
bpy.data.objects['DMaxDist'].modifiers['DT'].edge_mapping = 'NEAREST'
bpy.data.objects['DMaxDist'].modifiers['DT'].use_max_distance = True
bpy.data.objects['DMaxDist'].modifiers['DT'].max_distance = 0.05
