# WP 8.3.13: DATA_TRANSFER of UV maps (both maps by name) with every loop_mapping, from a
# ico sphere with random UVs and random seams (UV islands: the best island per face) to
# a rotated torus inside it (TOPOLOGY: the same ico sphere); a mix factor with a mask group.
# golden: startup=empty compare=evaluated:D0,evaluated:D1,evaluated:D2,evaluated:D3,evaluated:D4,evaluated:D5,evaluated:DMix tier=T2
import bpy
bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=2, radius=1.0)
bpy.context.object.name = 'S'
bpy.data.objects['S'].data.uv_layers['UVMap'].data.foreach_set('uv', [0.815, 0.917, 0.95, 0.909, 0.202, 0.171, 0.547, 0.759, 0.63, 0.418, 0.431, 0.759, 0.372, 0.908, 0.645, 0.212, 0.445, 0.073, 0.147, 0.685, 0.634, 0.304, 0.445, 0.749, 0.338, 0.233, 0.507, 0.185, 0.675, 0.806, 0.669, 0.834, 0.06, 0.676, 0.886, 0.806, 0.699, 0.427, 0.055, 0.545, 0.496, 0.342, 0.319, 0.338, 0.445, 0.264, 0.979, 0.066, 0.011, 0.942, 0.98, 0.162, 0.147, 0.644, 0.216, 0.908, 0.337, 0.393, 0.685, 0.164, 0.702, 0.363, 0.106, 0.524, 0.386, 0.945, 0.438, 0.507, 0.809, 0.399, 0.231, 0.248, 0.777, 0.327, 0.995, 0.173, 0.333, 0.934, 0.071, 0.035, 0.152, 0.139, 0.463, 0.125, 0.985, 0.108, 0.577, 0.73, 0.35, 0.046, 0.565, 0.51, 0.671, 0.998, 0.17, 0.797, 0.664, 0.872, 0.562, 0.56, 0.912, 0.294, 0.001, 0.097, 0.239, 0.64, 0.787, 0.621, 0.691, 0.704, 0.555, 0.759, 0.145, 0.545, 0.612, 0.267, 0.385, 0.777, 0.495, 0.651, 0.33, 0.585, 0.357, 0.976, 0.071, 0.311, 0.191, 0.685, 0.107, 0.629, 0.991, 0.343, 0.079, 0.288, 0.197, 0.406, 0.948, 0.179, 0.682, 0.943, 0.89, 0.138, 0.85, 0.332, 0.293, 0.206, 0.798, 0.827, 0.643, 0.428, 0.001, 0.515, 0.638, 0.562, 0.789, 0.723, 0.812, 0.214, 0.114, 0.245, 0.026, 0.381, 0.347, 0.743, 0.006, 0.217, 0.898, 0.225, 0.434, 0.706, 0.542, 0.635, 0.483, 0.082, 0.775, 0.988, 0.896, 0.458, 0.881, 0.448, 0.109, 0.781, 0.954, 0.725, 0.416, 0.657, 0.491, 0.167, 0.927, 0.966, 0.89, 0.886, 0.166, 0.323, 0.926, 0.135, 0.963, 0.351, 0.755, 0.627, 0.481, 0.264, 0.301, 0.86, 0.273, 0.009, 0.908, 0.908, 0.932, 0.311, 0.115, 0.783, 0.197, 0.123, 0.379, 0.074, 0.675, 0.988, 0.893, 0.407, 0.302, 0.063, 0.88, 0.444, 0.85, 0.791, 0.924, 0.343, 0.631, 0.932, 0.665, 0.236, 0.504, 0.933, 0.709, 0.51, 0.59, 0.632, 0.165, 0.495, 0.845, 0.152, 0.944, 0.041, 0.885, 0.165, 0.88, 0.029, 0.985, 0.673, 0.418, 0.381, 0.869, 0.339, 0.955, 0.824, 0.71, 0.104, 0.669, 0.602, 0.09, 0.886, 0.954, 0.473, 0.0, 0.09, 0.543, 0.75, 0.264, 0.614, 0.335, 0.719, 0.354, 0.278, 0.55, 0.533, 0.189, 0.197, 0.987, 0.778, 0.073, 0.236, 0.653, 0.876, 0.013, 0.432, 0.461, 0.906, 0.959, 0.418, 0.996, 0.389, 0.443, 0.335, 0.177, 0.86, 0.174, 0.796, 0.009, 0.086, 0.823, 0.715, 0.356, 0.397, 0.96, 0.291, 0.604, 0.805, 0.487, 0.88, 0.538, 0.084, 0.995, 0.608, 0.675, 0.86, 0.522, 0.987, 0.961, 0.64, 0.492, 0.725, 0.322, 0.309, 0.087, 0.902, 0.587, 0.555, 0.031, 0.512, 0.131, 0.474, 0.213, 0.66, 0.968, 0.868, 0.904, 0.278, 0.733, 0.451, 0.615, 0.086, 0.169, 0.784, 0.533, 0.124, 0.431, 0.851, 0.34, 0.367, 0.562, 0.589, 0.457, 0.507, 0.73, 0.459, 0.23, 0.029, 0.84, 0.543, 0.287, 0.284, 0.463, 0.8, 0.644, 0.967, 0.494, 0.06, 0.386, 0.083, 0.34, 0.826, 0.085, 0.542, 0.081, 0.19, 0.36, 0.512, 0.023, 0.455, 0.19, 0.201, 0.451, 0.159, 0.557, 0.883, 0.288, 0.076, 0.432, 0.567, 0.893, 0.154, 0.838, 0.099, 0.735, 0.337, 0.087, 0.605, 0.31, 0.623, 0.941, 0.081, 0.353, 0.097, 0.891, 0.621, 0.473, 0.641, 0.922, 0.025, 0.114, 0.183, 0.491, 0.557, 0.866, 0.164, 0.504, 0.446, 0.487, 0.491, 0.939, 0.628, 0.048, 0.825, 0.617, 0.995, 0.329, 0.853, 0.87, 0.336, 0.132, 0.806, 0.867, 0.69, 0.284, 0.804, 0.818, 0.175, 0.742, 0.249, 0.641, 1.0, 0.159, 0.034, 0.193, 0.17, 0.2, 0.729, 0.093, 0.244, 0.821, 0.14, 0.868, 0.937, 0.528, 0.604, 0.392, 0.665, 0.039, 0.54, 0.726, 0.044, 0.475, 0.584, 0.375, 0.509, 0.325, 0.105, 0.88, 0.81, 0.117, 0.697, 0.162, 0.169, 0.95, 0.559, 0.267, 0.309, 0.234, 0.107])
bpy.data.objects['S'].data.uv_layers.new(name='Second')
bpy.data.objects['S'].data.uv_layers['Second'].data.foreach_set('uv', [0.084, 0.707, 0.924, 1.428, 0.086, 1.552, -0.321, 1.996, -0.583, 0.091, -0.086, 0.156, 0.756, 1.517, 1.685, -0.746, 1.306, 1.657, -0.594, 0.463, -0.647, -0.244, -0.361, 0.428, -0.715, 0.376, 0.946, -0.192, 1.87, -0.423, 1.723, 0.031, -0.831, 1.677, 1.293, 0.454, -0.097, -0.97, 1.758, 1.738, 1.775, 1.71, 0.794, 1.332, -0.983, -0.891, -0.803, 0.62, 0.505, 1.963, 0.439, -0.258, 0.457, 0.504, -0.089, -0.545, -0.819, -0.575, 0.013, -0.664, 1.819, 0.919, 1.898, 1.802, 1.983, 1.785, 1.518, 0.871, 1.584, -0.18, 0.463, -0.138, 1.569, -0.421, 0.72, 0.201, 0.967, -0.981, 0.998, 1.169, 0.982, 0.925, 1.268, 0.402, 1.444, 1.416, 0.481, 1.978, 0.302, 1.555, -0.611, -0.664, 1.737, 1.953, 1.682, 0.29, 1.792, -0.213, -0.749, -0.763, -0.271, 0.283, -0.0, 0.771, 0.097, 0.352, 0.952, -0.153, -0.168, -0.82, 1.813, 2.0, 0.243, -0.976, -0.857, 0.266, -0.5, 1.2, 1.389, 0.046, 1.218, 0.045, 1.386, 1.943, 1.578, 1.413, 1.771, 1.409, 0.319, -0.607, -0.33, -0.217, 0.47, 0.141, 1.072, 0.272, -0.083, 1.366, 1.677, -0.77, -0.33, -0.774, 1.19, -0.27, 1.334, -0.72, 1.356, 1.802, -0.975, 0.05, 1.965, 1.676, 0.323, -0.752, -0.666, 0.906, 0.868, -0.065, 0.805, 0.946, 1.432, 1.132, -0.114, -0.144, 1.513, 1.501, 1.156, 0.063, 0.354, -0.441, 1.375, 1.318, 1.948, 1.095, 0.3, 0.91, 0.763, 0.708, 0.947, 1.51, 0.574, 0.339, -0.067, 1.12, -0.881, -0.589, 1.28, 0.412, -0.457, 0.868, -0.227, 0.413, 1.807, 0.504, 1.362, 0.529, 1.083, -0.158, 0.084, 0.332, 0.615, 1.243, 1.13, 0.289, 1.388, 0.686, -0.881, 1.573, 0.183, 0.01, 0.439, -0.733, -0.598, 0.919, -0.671, 0.837, 1.353, -0.787, 1.953, 1.769, 0.447, -0.998, 1.253, 1.85, -0.936, 1.904, 0.85, 0.98, -0.285, 0.992, 1.468, 1.061, 1.227, 1.512, 0.825, -0.652, 0.874, -0.389, 0.3, -0.611, -0.393, -0.076, -0.22, 0.346, 1.031, -0.45, 1.507, 0.411, 1.971, -0.341, -0.603, 1.682, 0.007, -0.639, -0.565, 0.545, 1.742, 1.345, 1.555, 1.106, 1.577, -0.887, 0.742, 1.962, -0.982, -0.592, 1.931, 0.912, 1.817, 1.01, -0.574, 1.892, 1.451, 1.629, -0.041, -0.3, 1.171, 1.879, 0.341, 1.184, 1.439, -0.809, 1.812, -0.043, 1.032, -0.322, -0.801, 0.884, -0.996, 1.505, 1.506, 1.601, -0.551, -0.436, 1.307, 0.241, -0.264, 1.748, -0.13, -0.096, 0.131, -0.351, 1.799, 1.798, 0.205, 0.396, 0.944, -0.862, 1.327, 0.914, 0.381, 0.662, 1.342, 0.596, -0.956, -0.765, 1.472, 0.084, 1.256, 1.935, 0.57, -0.942, -0.389, 0.604, 1.521, -0.003, 1.668, -0.286, 0.416, 0.385, -0.402, 0.267, 1.339, 0.179, -0.289, 1.007, 0.025, 1.76, 1.454, -0.295, 1.605, 0.196, 1.531, 1.392, 1.366, -0.063, 1.706, 1.619, 1.571, 0.235, 1.359, 0.17, 1.746, -0.375, 1.019, 1.061, 0.123, 0.624, 0.167, -0.831, 1.727, 1.618, 0.096, -0.497, 1.874, 1.493, -0.23, -0.925, -0.108, -0.404, 0.911, -0.206, -0.492, -0.455, 0.133, 0.333, -0.388, -0.346, -0.472, -0.727, -0.678, -0.574, 0.209, 1.081, 1.328, -0.799, 1.91, 1.293, 0.839, 0.396, -0.174, 0.493, 1.187, 0.083, -0.531, 0.4, -0.636, 0.95, 1.24, 0.565, -0.343, -0.536, -0.03, -0.479, -0.7, 1.702, 0.914, 1.873, 0.247, -0.18, -0.396, -0.449, 1.93, -0.5, 0.318, 0.862, 0.764, 1.44, 1.98, 0.015, 0.012, 1.341, 1.737, -0.155, 1.745, -0.65, -0.847, 0.004, 0.88, 1.443, 1.004, 0.455, 1.139, -0.089, -0.56, 1.346, 0.234, 1.854, 0.244, 0.96, 0.593, 1.522, -0.71, -0.669, 1.669, -0.344, -0.538, 1.522, 0.25, 0.746, -0.412, 1.483, -0.444, -0.999, 1.163, 0.396, -0.885, 0.069, 1.889, -0.435, 0.377, -0.675, -0.263, 0.583, 0.914, 1.738, 0.801, -0.677, 1.441, 1.739, 0.988, 0.969, -0.595, -0.078, 1.787, 0.241, 0.953, -0.249, 1.111, 0.676])
bpy.data.objects['S'].data.attributes.new(name='uv_seam', type='BOOLEAN', domain='EDGE')
bpy.data.objects['S'].data.attributes['uv_seam'].data.foreach_set('value', [False, False, False, False, False, False, False, True, False, True, False, True, True, True, False, False, True, False, False, False, True, False, False, False, False, False, False, False, True, False, False, False, False, True, True, False, False, False, True, False, True, False, False, True, False, False, False, False, False, False, True, False, False, True, True, False, False, False, False, False, False, False, False, False, False, False, False, False, False, True, False, False, False, True, True, False, False, False, False, False, False, False, True, False, True, False, False, False, False, False, False, False, True, False, False, False, False, False, False, True, False, False, False, False, False, False, True, True, False, False, False, True, True, False, True, True, True, False, False, False])
bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=2, radius=1.0)
bpy.context.object.name = 'D0'
bpy.data.objects['D0'].location = (0.0, 0.0, 0.2)
bpy.ops.object.shade_smooth()
bpy.data.objects['D0'].data.uv_layers.new(name='Second')
bpy.data.objects['D0'].modifiers.new(name='DT', type='DATA_TRANSFER')
bpy.data.objects['D0'].modifiers['DT'].object = bpy.data.objects['S']
bpy.data.objects['D0'].modifiers['DT'].use_loop_data = True
bpy.data.objects['D0'].modifiers['DT'].data_types_loops = {'UV'}
bpy.data.objects['D0'].modifiers['DT'].loop_mapping = 'TOPOLOGY'
bpy.data.objects['D0'].modifiers['DT'].use_object_transform = False
bpy.ops.mesh.primitive_torus_add(major_segments=16, minor_segments=8, major_radius=0.55, minor_radius=0.2, location=(0.031, -0.017, 0.043), rotation=(0.13, 0.27, 0.41))
bpy.context.object.name = 'D1'
bpy.ops.object.shade_smooth()
bpy.data.objects['D1'].data.uv_layers.new(name='Second')
bpy.data.objects['D1'].modifiers.new(name='DT', type='DATA_TRANSFER')
bpy.data.objects['D1'].modifiers['DT'].object = bpy.data.objects['S']
bpy.data.objects['D1'].modifiers['DT'].use_loop_data = True
bpy.data.objects['D1'].modifiers['DT'].data_types_loops = {'UV'}
bpy.data.objects['D1'].modifiers['DT'].loop_mapping = 'NEAREST_NORMAL'
bpy.ops.mesh.primitive_torus_add(major_segments=16, minor_segments=8, major_radius=0.55, minor_radius=0.2, location=(0.031, -0.017, 0.043), rotation=(0.13, 0.27, 0.41))
bpy.context.object.name = 'D2'
bpy.ops.object.shade_smooth()
bpy.data.objects['D2'].data.uv_layers.new(name='Second')
bpy.data.objects['D2'].modifiers.new(name='DT', type='DATA_TRANSFER')
bpy.data.objects['D2'].modifiers['DT'].object = bpy.data.objects['S']
bpy.data.objects['D2'].modifiers['DT'].use_loop_data = True
bpy.data.objects['D2'].modifiers['DT'].data_types_loops = {'UV'}
bpy.data.objects['D2'].modifiers['DT'].loop_mapping = 'NEAREST_POLYNOR'
bpy.ops.mesh.primitive_torus_add(major_segments=16, minor_segments=8, major_radius=0.55, minor_radius=0.2, location=(0.031, -0.017, 0.043), rotation=(0.13, 0.27, 0.41))
bpy.context.object.name = 'D3'
bpy.ops.object.shade_smooth()
bpy.data.objects['D3'].data.uv_layers.new(name='Second')
bpy.data.objects['D3'].modifiers.new(name='DT', type='DATA_TRANSFER')
bpy.data.objects['D3'].modifiers['DT'].object = bpy.data.objects['S']
bpy.data.objects['D3'].modifiers['DT'].use_loop_data = True
bpy.data.objects['D3'].modifiers['DT'].data_types_loops = {'UV'}
bpy.data.objects['D3'].modifiers['DT'].loop_mapping = 'NEAREST_POLY'
bpy.ops.mesh.primitive_torus_add(major_segments=16, minor_segments=8, major_radius=0.55, minor_radius=0.2, location=(0.031, -0.017, 0.043), rotation=(0.13, 0.27, 0.41))
bpy.context.object.name = 'D4'
bpy.ops.object.shade_smooth()
bpy.data.objects['D4'].data.uv_layers.new(name='Second')
bpy.data.objects['D4'].modifiers.new(name='DT', type='DATA_TRANSFER')
bpy.data.objects['D4'].modifiers['DT'].object = bpy.data.objects['S']
bpy.data.objects['D4'].modifiers['DT'].use_loop_data = True
bpy.data.objects['D4'].modifiers['DT'].data_types_loops = {'UV'}
bpy.data.objects['D4'].modifiers['DT'].loop_mapping = 'POLYINTERP_NEAREST'
bpy.ops.mesh.primitive_torus_add(major_segments=16, minor_segments=8, major_radius=0.55, minor_radius=0.2, location=(0.031, -0.017, 0.043), rotation=(0.13, 0.27, 0.41))
bpy.context.object.name = 'D5'
bpy.ops.object.shade_smooth()
bpy.data.objects['D5'].data.uv_layers.new(name='Second')
bpy.data.objects['D5'].modifiers.new(name='DT', type='DATA_TRANSFER')
bpy.data.objects['D5'].modifiers['DT'].object = bpy.data.objects['S']
bpy.data.objects['D5'].modifiers['DT'].use_loop_data = True
bpy.data.objects['D5'].modifiers['DT'].data_types_loops = {'UV'}
bpy.data.objects['D5'].modifiers['DT'].loop_mapping = 'POLYINTERP_LNORPROJ'
bpy.data.objects['D5'].modifiers['DT'].ray_radius = 0.02
bpy.ops.mesh.primitive_torus_add(major_segments=16, minor_segments=8, major_radius=0.55, minor_radius=0.2, location=(0.031, -0.017, 0.043), rotation=(0.13, 0.27, 0.41))
bpy.context.object.name = 'DMix'
bpy.data.objects['DMix'].vertex_groups.new(name='M')
bpy.data.objects['DMix'].vertex_groups['M'].add([21, 33, 49, 121, 123], 0.1, 'REPLACE')
bpy.data.objects['DMix'].vertex_groups['M'].add([3, 26, 30, 38, 52, 56, 59, 66, 76, 80, 85], 0.2, 'REPLACE')
bpy.data.objects['DMix'].vertex_groups['M'].add([5, 11, 13, 16, 46, 55, 64, 78, 95, 97, 101, 103, 112, 113, 126], 0.3, 'REPLACE')
bpy.data.objects['DMix'].vertex_groups['M'].add([4, 29, 34, 58, 63, 86, 92, 99, 124], 0.4, 'REPLACE')
bpy.data.objects['DMix'].vertex_groups['M'].add([0, 10, 17, 25, 27, 42, 43, 45, 51, 62, 69, 70, 77, 98, 100, 104, 106, 110, 115], 0.5, 'REPLACE')
bpy.data.objects['DMix'].vertex_groups['M'].add([12, 41, 60, 68, 114], 0.6, 'REPLACE')
bpy.data.objects['DMix'].vertex_groups['M'].add([19, 22, 47, 48, 57, 118], 0.7, 'REPLACE')
bpy.data.objects['DMix'].vertex_groups['M'].add([1, 31, 37, 40, 61, 71, 72, 75, 127], 0.8, 'REPLACE')
bpy.data.objects['DMix'].vertex_groups['M'].add([2, 6, 8, 15, 28, 32, 36, 39, 88, 125], 0.9, 'REPLACE')
bpy.data.objects['DMix'].vertex_groups['M'].add([7, 23, 44, 50, 65, 74, 79, 82, 83, 93, 96, 108, 111, 117, 119], 1.0, 'REPLACE')
bpy.data.objects['DMix'].modifiers.new(name='DT', type='DATA_TRANSFER')
bpy.data.objects['DMix'].modifiers['DT'].object = bpy.data.objects['S']
bpy.data.objects['DMix'].modifiers['DT'].use_loop_data = True
bpy.data.objects['DMix'].modifiers['DT'].data_types_loops = {'UV'}
bpy.data.objects['DMix'].modifiers['DT'].loop_mapping = 'POLYINTERP_NEAREST'
bpy.data.objects['DMix'].modifiers['DT'].mix_factor = 0.4
bpy.data.objects['DMix'].modifiers['DT'].vertex_group = 'M'
