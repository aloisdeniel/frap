# WP 8.3.13: VERTEX_WEIGHT_PROXIMITY: OBJECT distance (to a mesh and to an empty), the
# GEOMETRY modes (nearest vertex, edge, face and all three of a tilted plane; no geometry
# kind and an empty target: distance to the target's origin), an inverted and an empty
# distance range, a falloff, normalize, the influence with a mask group, no target. Only
# the group's vertices change (the membership probe adds the others at 0.333).
# golden: startup=empty compare=evaluated:PObject,evaluated:PObjectEmpty,evaluated:PVert,evaluated:PEdge,evaluated:PFace,evaluated:PAll,evaluated:PNone,evaluated:PGeomEmpty,evaluated:PInverted,evaluated:PStep,evaluated:PSmooth,evaluated:PNorm,evaluated:PMask,evaluated:PNoTarget tier=T2
import bpy
bpy.ops.mesh.primitive_plane_add(size=1.2, location=(0.3, 0.2, 0.8), rotation=(0.4, 0.2, 0.0))
bpy.context.object.name = 'T'
bpy.ops.object.empty_add(location=(-0.5, 0.4, 0.6))
bpy.context.object.name = 'TE'
bpy.ops.mesh.primitive_grid_add(x_subdivisions=7, y_subdivisions=7, size=2)
bpy.context.object.name = 'PObject'
bpy.data.objects['PObject'].vertex_groups.new(name='G')
bpy.data.objects['PObject'].vertex_groups['G'].add([3, 26, 49, 53], 0.1, 'REPLACE')
bpy.data.objects['PObject'].vertex_groups['G'].add([11, 14, 19, 58], 0.2, 'REPLACE')
bpy.data.objects['PObject'].vertex_groups['G'].add([2, 61], 0.3, 'REPLACE')
bpy.data.objects['PObject'].vertex_groups['G'].add([30, 35, 39], 0.4, 'REPLACE')
bpy.data.objects['PObject'].vertex_groups['G'].add([16, 21, 28, 63], 0.5, 'REPLACE')
bpy.data.objects['PObject'].vertex_groups['G'].add([5, 9, 22, 29, 52, 54], 0.6, 'REPLACE')
bpy.data.objects['PObject'].vertex_groups['G'].add([8, 12, 20, 24, 40], 0.7, 'REPLACE')
bpy.data.objects['PObject'].vertex_groups['G'].add([1, 32, 45], 0.8, 'REPLACE')
bpy.data.objects['PObject'].vertex_groups['G'].add([7, 23, 42, 44, 48, 56, 59], 0.9, 'REPLACE')
bpy.data.objects['PObject'].vertex_groups['G'].add([33, 36, 38, 43], 1.0, 'REPLACE')
bpy.data.objects['PObject'].modifiers.new(name='W', type='VERTEX_WEIGHT_PROXIMITY')
bpy.data.objects['PObject'].modifiers['W'].vertex_group = 'G'
bpy.data.objects['PObject'].modifiers['W'].proximity_mode = 'OBJECT'
bpy.data.objects['PObject'].modifiers['W'].target = bpy.data.objects['T']
bpy.data.objects['PObject'].modifiers['W'].max_dist = 2.0
bpy.data.objects['PObject'].modifiers.new(name='Probe', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['PObject'].modifiers['Probe'].vertex_group = 'G'
bpy.data.objects['PObject'].modifiers['Probe'].use_add = True
bpy.data.objects['PObject'].modifiers['Probe'].add_threshold = 0.0
bpy.data.objects['PObject'].modifiers['Probe'].default_weight = 0.333
bpy.data.objects['PObject'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['PObject'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['PObject'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['PObject'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['PObject'].modifiers['Dz'].strength = 1.0
bpy.ops.mesh.primitive_grid_add(x_subdivisions=7, y_subdivisions=7, size=2)
bpy.context.object.name = 'PObjectEmpty'
bpy.data.objects['PObjectEmpty'].vertex_groups.new(name='G')
bpy.data.objects['PObjectEmpty'].vertex_groups['G'].add([14, 28], 0.1, 'REPLACE')
bpy.data.objects['PObjectEmpty'].vertex_groups['G'].add([13, 15, 18, 30, 36, 56, 58, 63], 0.2, 'REPLACE')
bpy.data.objects['PObjectEmpty'].vertex_groups['G'].add([10, 16, 26, 34, 37, 43, 62], 0.3, 'REPLACE')
bpy.data.objects['PObjectEmpty'].vertex_groups['G'].add([12, 21, 22, 29, 33, 55, 60], 0.4, 'REPLACE')
bpy.data.objects['PObjectEmpty'].vertex_groups['G'].add([11, 17, 45, 46, 51], 0.5, 'REPLACE')
bpy.data.objects['PObjectEmpty'].vertex_groups['G'].add([47, 57], 0.6, 'REPLACE')
bpy.data.objects['PObjectEmpty'].vertex_groups['G'].add([9, 19, 42], 0.7, 'REPLACE')
bpy.data.objects['PObjectEmpty'].vertex_groups['G'].add([31, 32, 59], 0.8, 'REPLACE')
bpy.data.objects['PObjectEmpty'].vertex_groups['G'].add([4, 27, 41], 0.9, 'REPLACE')
bpy.data.objects['PObjectEmpty'].vertex_groups['G'].add([1, 6, 20, 38, 48, 52], 1.0, 'REPLACE')
bpy.data.objects['PObjectEmpty'].modifiers.new(name='W', type='VERTEX_WEIGHT_PROXIMITY')
bpy.data.objects['PObjectEmpty'].modifiers['W'].vertex_group = 'G'
bpy.data.objects['PObjectEmpty'].modifiers['W'].proximity_mode = 'OBJECT'
bpy.data.objects['PObjectEmpty'].modifiers['W'].target = bpy.data.objects['TE']
bpy.data.objects['PObjectEmpty'].modifiers['W'].min_dist = 0.5
bpy.data.objects['PObjectEmpty'].modifiers['W'].max_dist = 1.5
bpy.data.objects['PObjectEmpty'].modifiers.new(name='Probe', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['PObjectEmpty'].modifiers['Probe'].vertex_group = 'G'
bpy.data.objects['PObjectEmpty'].modifiers['Probe'].use_add = True
bpy.data.objects['PObjectEmpty'].modifiers['Probe'].add_threshold = 0.0
bpy.data.objects['PObjectEmpty'].modifiers['Probe'].default_weight = 0.333
bpy.data.objects['PObjectEmpty'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['PObjectEmpty'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['PObjectEmpty'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['PObjectEmpty'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['PObjectEmpty'].modifiers['Dz'].strength = 1.0
bpy.ops.mesh.primitive_grid_add(x_subdivisions=7, y_subdivisions=7, size=2)
bpy.context.object.name = 'PVert'
bpy.data.objects['PVert'].vertex_groups.new(name='G')
bpy.data.objects['PVert'].vertex_groups['G'].add([6, 14, 26, 35, 38], 0.1, 'REPLACE')
bpy.data.objects['PVert'].vertex_groups['G'].add([8, 19, 32, 40, 49, 50, 52, 62], 0.2, 'REPLACE')
bpy.data.objects['PVert'].vertex_groups['G'].add([1, 45], 0.3, 'REPLACE')
bpy.data.objects['PVert'].vertex_groups['G'].add([5, 33, 47, 60], 0.4, 'REPLACE')
bpy.data.objects['PVert'].vertex_groups['G'].add([3, 11, 34, 39, 54], 0.5, 'REPLACE')
bpy.data.objects['PVert'].vertex_groups['G'].add([9, 17, 23, 31, 43, 46], 0.6, 'REPLACE')
bpy.data.objects['PVert'].vertex_groups['G'].add([0, 44], 0.7, 'REPLACE')
bpy.data.objects['PVert'].vertex_groups['G'].add([21, 22, 51, 57], 0.8, 'REPLACE')
bpy.data.objects['PVert'].vertex_groups['G'].add([15, 27, 29, 41], 0.9, 'REPLACE')
bpy.data.objects['PVert'].vertex_groups['G'].add([10, 25, 37, 48, 56], 1.0, 'REPLACE')
bpy.data.objects['PVert'].modifiers.new(name='W', type='VERTEX_WEIGHT_PROXIMITY')
bpy.data.objects['PVert'].modifiers['W'].vertex_group = 'G'
bpy.data.objects['PVert'].modifiers['W'].proximity_mode = 'GEOMETRY'
bpy.data.objects['PVert'].modifiers['W'].proximity_geometry = {'VERTEX'}
bpy.data.objects['PVert'].modifiers['W'].target = bpy.data.objects['T']
bpy.data.objects['PVert'].modifiers['W'].max_dist = 1.5
bpy.data.objects['PVert'].modifiers.new(name='Probe', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['PVert'].modifiers['Probe'].vertex_group = 'G'
bpy.data.objects['PVert'].modifiers['Probe'].use_add = True
bpy.data.objects['PVert'].modifiers['Probe'].add_threshold = 0.0
bpy.data.objects['PVert'].modifiers['Probe'].default_weight = 0.333
bpy.data.objects['PVert'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['PVert'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['PVert'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['PVert'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['PVert'].modifiers['Dz'].strength = 1.0
bpy.ops.mesh.primitive_grid_add(x_subdivisions=7, y_subdivisions=7, size=2)
bpy.context.object.name = 'PEdge'
bpy.data.objects['PEdge'].vertex_groups.new(name='G')
bpy.data.objects['PEdge'].vertex_groups['G'].add([4, 30, 34, 63], 0.1, 'REPLACE')
bpy.data.objects['PEdge'].vertex_groups['G'].add([7, 11, 41, 52, 54, 61], 0.2, 'REPLACE')
bpy.data.objects['PEdge'].vertex_groups['G'].add([8, 9, 31, 37], 0.3, 'REPLACE')
bpy.data.objects['PEdge'].vertex_groups['G'].add([1, 3, 13, 28, 60], 0.4, 'REPLACE')
bpy.data.objects['PEdge'].vertex_groups['G'].add([14, 15, 35, 45, 57], 0.5, 'REPLACE')
bpy.data.objects['PEdge'].vertex_groups['G'].add([6, 18, 21, 42], 0.6, 'REPLACE')
bpy.data.objects['PEdge'].vertex_groups['G'].add([44], 0.7, 'REPLACE')
bpy.data.objects['PEdge'].vertex_groups['G'].add([0, 20, 29, 43, 48, 53, 59], 0.8, 'REPLACE')
bpy.data.objects['PEdge'].vertex_groups['G'].add([36, 62], 0.9, 'REPLACE')
bpy.data.objects['PEdge'].vertex_groups['G'].add([2, 5, 16, 17, 49, 55], 1.0, 'REPLACE')
bpy.data.objects['PEdge'].modifiers.new(name='W', type='VERTEX_WEIGHT_PROXIMITY')
bpy.data.objects['PEdge'].modifiers['W'].vertex_group = 'G'
bpy.data.objects['PEdge'].modifiers['W'].proximity_mode = 'GEOMETRY'
bpy.data.objects['PEdge'].modifiers['W'].proximity_geometry = {'EDGE'}
bpy.data.objects['PEdge'].modifiers['W'].target = bpy.data.objects['T']
bpy.data.objects['PEdge'].modifiers['W'].max_dist = 1.5
bpy.data.objects['PEdge'].modifiers.new(name='Probe', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['PEdge'].modifiers['Probe'].vertex_group = 'G'
bpy.data.objects['PEdge'].modifiers['Probe'].use_add = True
bpy.data.objects['PEdge'].modifiers['Probe'].add_threshold = 0.0
bpy.data.objects['PEdge'].modifiers['Probe'].default_weight = 0.333
bpy.data.objects['PEdge'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['PEdge'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['PEdge'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['PEdge'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['PEdge'].modifiers['Dz'].strength = 1.0
bpy.ops.mesh.primitive_grid_add(x_subdivisions=7, y_subdivisions=7, size=2)
bpy.context.object.name = 'PFace'
bpy.data.objects['PFace'].vertex_groups.new(name='G')
bpy.data.objects['PFace'].vertex_groups['G'].add([15, 17, 32, 48, 49, 59], 0.1, 'REPLACE')
bpy.data.objects['PFace'].vertex_groups['G'].add([8, 33, 35, 39, 56, 62], 0.2, 'REPLACE')
bpy.data.objects['PFace'].vertex_groups['G'].add([10, 23, 41, 63], 0.3, 'REPLACE')
bpy.data.objects['PFace'].vertex_groups['G'].add([38, 47], 0.4, 'REPLACE')
bpy.data.objects['PFace'].vertex_groups['G'].add([7, 28, 55, 58, 61], 0.5, 'REPLACE')
bpy.data.objects['PFace'].vertex_groups['G'].add([4, 43], 0.6, 'REPLACE')
bpy.data.objects['PFace'].vertex_groups['G'].add([24, 29, 57], 0.7, 'REPLACE')
bpy.data.objects['PFace'].vertex_groups['G'].add([42, 50, 51, 60], 0.8, 'REPLACE')
bpy.data.objects['PFace'].vertex_groups['G'].add([11, 14, 37, 52, 53], 0.9, 'REPLACE')
bpy.data.objects['PFace'].vertex_groups['G'].add([0, 2, 12, 13, 18, 27], 1.0, 'REPLACE')
bpy.data.objects['PFace'].modifiers.new(name='W', type='VERTEX_WEIGHT_PROXIMITY')
bpy.data.objects['PFace'].modifiers['W'].vertex_group = 'G'
bpy.data.objects['PFace'].modifiers['W'].proximity_mode = 'GEOMETRY'
bpy.data.objects['PFace'].modifiers['W'].proximity_geometry = {'FACE'}
bpy.data.objects['PFace'].modifiers['W'].target = bpy.data.objects['T']
bpy.data.objects['PFace'].modifiers['W'].max_dist = 1.5
bpy.data.objects['PFace'].modifiers.new(name='Probe', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['PFace'].modifiers['Probe'].vertex_group = 'G'
bpy.data.objects['PFace'].modifiers['Probe'].use_add = True
bpy.data.objects['PFace'].modifiers['Probe'].add_threshold = 0.0
bpy.data.objects['PFace'].modifiers['Probe'].default_weight = 0.333
bpy.data.objects['PFace'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['PFace'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['PFace'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['PFace'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['PFace'].modifiers['Dz'].strength = 1.0
bpy.ops.mesh.primitive_grid_add(x_subdivisions=7, y_subdivisions=7, size=2)
bpy.context.object.name = 'PAll'
bpy.data.objects['PAll'].vertex_groups.new(name='G')
bpy.data.objects['PAll'].vertex_groups['G'].add([2, 22, 52], 0.1, 'REPLACE')
bpy.data.objects['PAll'].vertex_groups['G'].add([9, 13, 14, 16, 25, 26, 35, 47], 0.2, 'REPLACE')
bpy.data.objects['PAll'].vertex_groups['G'].add([38, 41], 0.3, 'REPLACE')
bpy.data.objects['PAll'].vertex_groups['G'].add([1, 46, 53], 0.4, 'REPLACE')
bpy.data.objects['PAll'].vertex_groups['G'].add([0, 5, 8, 27, 30, 45, 51, 62], 0.5, 'REPLACE')
bpy.data.objects['PAll'].vertex_groups['G'].add([3, 10, 19, 37, 48, 50], 0.6, 'REPLACE')
bpy.data.objects['PAll'].vertex_groups['G'].add([11, 18, 44, 56, 61], 0.7, 'REPLACE')
bpy.data.objects['PAll'].vertex_groups['G'].add([21, 33], 0.8, 'REPLACE')
bpy.data.objects['PAll'].vertex_groups['G'].add([32, 34, 55, 60], 0.9, 'REPLACE')
bpy.data.objects['PAll'].modifiers.new(name='W', type='VERTEX_WEIGHT_PROXIMITY')
bpy.data.objects['PAll'].modifiers['W'].vertex_group = 'G'
bpy.data.objects['PAll'].modifiers['W'].proximity_mode = 'GEOMETRY'
bpy.data.objects['PAll'].modifiers['W'].proximity_geometry = {'VERTEX', 'EDGE', 'FACE'}
bpy.data.objects['PAll'].modifiers['W'].target = bpy.data.objects['T']
bpy.data.objects['PAll'].modifiers['W'].min_dist = 0.2
bpy.data.objects['PAll'].modifiers['W'].max_dist = 1.2
bpy.data.objects['PAll'].modifiers.new(name='Probe', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['PAll'].modifiers['Probe'].vertex_group = 'G'
bpy.data.objects['PAll'].modifiers['Probe'].use_add = True
bpy.data.objects['PAll'].modifiers['Probe'].add_threshold = 0.0
bpy.data.objects['PAll'].modifiers['Probe'].default_weight = 0.333
bpy.data.objects['PAll'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['PAll'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['PAll'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['PAll'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['PAll'].modifiers['Dz'].strength = 1.0
bpy.ops.mesh.primitive_grid_add(x_subdivisions=7, y_subdivisions=7, size=2)
bpy.context.object.name = 'PNone'
bpy.data.objects['PNone'].vertex_groups.new(name='G')
bpy.data.objects['PNone'].vertex_groups['G'].add([2, 7, 17, 45], 0.1, 'REPLACE')
bpy.data.objects['PNone'].vertex_groups['G'].add([4, 11, 29, 37, 42, 49, 63], 0.2, 'REPLACE')
bpy.data.objects['PNone'].vertex_groups['G'].add([0], 0.3, 'REPLACE')
bpy.data.objects['PNone'].vertex_groups['G'].add([6, 8, 24, 31, 34, 39, 51, 56, 60], 0.5, 'REPLACE')
bpy.data.objects['PNone'].vertex_groups['G'].add([62], 0.6, 'REPLACE')
bpy.data.objects['PNone'].vertex_groups['G'].add([1, 9, 15, 22, 35, 59], 0.7, 'REPLACE')
bpy.data.objects['PNone'].vertex_groups['G'].add([5, 25, 40, 47], 0.8, 'REPLACE')
bpy.data.objects['PNone'].vertex_groups['G'].add([14, 16, 20, 55, 61], 0.9, 'REPLACE')
bpy.data.objects['PNone'].vertex_groups['G'].add([10, 27, 41, 43, 52], 1.0, 'REPLACE')
bpy.data.objects['PNone'].modifiers.new(name='W', type='VERTEX_WEIGHT_PROXIMITY')
bpy.data.objects['PNone'].modifiers['W'].vertex_group = 'G'
bpy.data.objects['PNone'].modifiers['W'].proximity_mode = 'GEOMETRY'
bpy.data.objects['PNone'].modifiers['W'].proximity_geometry = set()
bpy.data.objects['PNone'].modifiers['W'].target = bpy.data.objects['T']
bpy.data.objects['PNone'].modifiers['W'].max_dist = 2.0
bpy.data.objects['PNone'].modifiers.new(name='Probe', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['PNone'].modifiers['Probe'].vertex_group = 'G'
bpy.data.objects['PNone'].modifiers['Probe'].use_add = True
bpy.data.objects['PNone'].modifiers['Probe'].add_threshold = 0.0
bpy.data.objects['PNone'].modifiers['Probe'].default_weight = 0.333
bpy.data.objects['PNone'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['PNone'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['PNone'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['PNone'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['PNone'].modifiers['Dz'].strength = 1.0
bpy.ops.mesh.primitive_grid_add(x_subdivisions=7, y_subdivisions=7, size=2)
bpy.context.object.name = 'PGeomEmpty'
bpy.data.objects['PGeomEmpty'].vertex_groups.new(name='G')
bpy.data.objects['PGeomEmpty'].vertex_groups['G'].add([1, 6, 13, 27, 28, 48, 54], 0.1, 'REPLACE')
bpy.data.objects['PGeomEmpty'].vertex_groups['G'].add([15, 39, 40, 41, 47], 0.2, 'REPLACE')
bpy.data.objects['PGeomEmpty'].vertex_groups['G'].add([9, 56], 0.3, 'REPLACE')
bpy.data.objects['PGeomEmpty'].vertex_groups['G'].add([55], 0.4, 'REPLACE')
bpy.data.objects['PGeomEmpty'].vertex_groups['G'].add([22, 38, 43, 46, 60], 0.5, 'REPLACE')
bpy.data.objects['PGeomEmpty'].vertex_groups['G'].add([18, 26, 34, 37, 58, 59, 61], 0.6, 'REPLACE')
bpy.data.objects['PGeomEmpty'].vertex_groups['G'].add([2, 12, 16, 25, 33], 0.7, 'REPLACE')
bpy.data.objects['PGeomEmpty'].vertex_groups['G'].add([10, 19, 21, 23, 62], 0.8, 'REPLACE')
bpy.data.objects['PGeomEmpty'].vertex_groups['G'].add([5, 32, 35, 36, 49, 50, 57], 0.9, 'REPLACE')
bpy.data.objects['PGeomEmpty'].vertex_groups['G'].add([8, 52, 63], 1.0, 'REPLACE')
bpy.data.objects['PGeomEmpty'].modifiers.new(name='W', type='VERTEX_WEIGHT_PROXIMITY')
bpy.data.objects['PGeomEmpty'].modifiers['W'].vertex_group = 'G'
bpy.data.objects['PGeomEmpty'].modifiers['W'].proximity_mode = 'GEOMETRY'
bpy.data.objects['PGeomEmpty'].modifiers['W'].proximity_geometry = {'FACE'}
bpy.data.objects['PGeomEmpty'].modifiers['W'].target = bpy.data.objects['TE']
bpy.data.objects['PGeomEmpty'].modifiers['W'].max_dist = 2.0
bpy.data.objects['PGeomEmpty'].modifiers.new(name='Probe', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['PGeomEmpty'].modifiers['Probe'].vertex_group = 'G'
bpy.data.objects['PGeomEmpty'].modifiers['Probe'].use_add = True
bpy.data.objects['PGeomEmpty'].modifiers['Probe'].add_threshold = 0.0
bpy.data.objects['PGeomEmpty'].modifiers['Probe'].default_weight = 0.333
bpy.data.objects['PGeomEmpty'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['PGeomEmpty'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['PGeomEmpty'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['PGeomEmpty'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['PGeomEmpty'].modifiers['Dz'].strength = 1.0
bpy.ops.mesh.primitive_grid_add(x_subdivisions=7, y_subdivisions=7, size=2)
bpy.context.object.name = 'PInverted'
bpy.data.objects['PInverted'].vertex_groups.new(name='G')
bpy.data.objects['PInverted'].vertex_groups['G'].add([44, 53], 0.1, 'REPLACE')
bpy.data.objects['PInverted'].vertex_groups['G'].add([12, 27], 0.2, 'REPLACE')
bpy.data.objects['PInverted'].vertex_groups['G'].add([41], 0.3, 'REPLACE')
bpy.data.objects['PInverted'].vertex_groups['G'].add([23, 29, 39, 51, 55], 0.4, 'REPLACE')
bpy.data.objects['PInverted'].vertex_groups['G'].add([37, 45, 59], 0.5, 'REPLACE')
bpy.data.objects['PInverted'].vertex_groups['G'].add([18, 25, 56], 0.6, 'REPLACE')
bpy.data.objects['PInverted'].vertex_groups['G'].add([3, 11, 40, 48, 58], 0.7, 'REPLACE')
bpy.data.objects['PInverted'].vertex_groups['G'].add([14, 26, 34, 49], 0.8, 'REPLACE')
bpy.data.objects['PInverted'].vertex_groups['G'].add([0, 5, 16, 21, 22, 32, 33, 35], 0.9, 'REPLACE')
bpy.data.objects['PInverted'].vertex_groups['G'].add([10, 19, 36, 61, 63], 1.0, 'REPLACE')
bpy.data.objects['PInverted'].modifiers.new(name='W', type='VERTEX_WEIGHT_PROXIMITY')
bpy.data.objects['PInverted'].modifiers['W'].vertex_group = 'G'
bpy.data.objects['PInverted'].modifiers['W'].proximity_mode = 'GEOMETRY'
bpy.data.objects['PInverted'].modifiers['W'].proximity_geometry = {'FACE'}
bpy.data.objects['PInverted'].modifiers['W'].target = bpy.data.objects['T']
bpy.data.objects['PInverted'].modifiers['W'].min_dist = 1.2
bpy.data.objects['PInverted'].modifiers['W'].max_dist = 0.3
bpy.data.objects['PInverted'].modifiers.new(name='Probe', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['PInverted'].modifiers['Probe'].vertex_group = 'G'
bpy.data.objects['PInverted'].modifiers['Probe'].use_add = True
bpy.data.objects['PInverted'].modifiers['Probe'].add_threshold = 0.0
bpy.data.objects['PInverted'].modifiers['Probe'].default_weight = 0.333
bpy.data.objects['PInverted'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['PInverted'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['PInverted'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['PInverted'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['PInverted'].modifiers['Dz'].strength = 1.0
bpy.ops.mesh.primitive_grid_add(x_subdivisions=7, y_subdivisions=7, size=2)
bpy.context.object.name = 'PStep'
bpy.data.objects['PStep'].vertex_groups.new(name='G')
bpy.data.objects['PStep'].vertex_groups['G'].add([8, 9, 13, 25, 28, 36, 52], 0.1, 'REPLACE')
bpy.data.objects['PStep'].vertex_groups['G'].add([32, 46, 47, 59], 0.2, 'REPLACE')
bpy.data.objects['PStep'].vertex_groups['G'].add([23, 62], 0.3, 'REPLACE')
bpy.data.objects['PStep'].vertex_groups['G'].add([34, 39, 41, 42, 53, 54, 56], 0.4, 'REPLACE')
bpy.data.objects['PStep'].vertex_groups['G'].add([0, 4, 6, 16, 29], 0.5, 'REPLACE')
bpy.data.objects['PStep'].vertex_groups['G'].add([1, 7, 11, 12, 26, 45], 0.6, 'REPLACE')
bpy.data.objects['PStep'].vertex_groups['G'].add([30, 60], 0.7, 'REPLACE')
bpy.data.objects['PStep'].vertex_groups['G'].add([5, 15, 31, 37, 43, 51], 0.8, 'REPLACE')
bpy.data.objects['PStep'].vertex_groups['G'].add([21, 61, 63], 0.9, 'REPLACE')
bpy.data.objects['PStep'].vertex_groups['G'].add([14, 33, 35, 48, 49], 1.0, 'REPLACE')
bpy.data.objects['PStep'].modifiers.new(name='W', type='VERTEX_WEIGHT_PROXIMITY')
bpy.data.objects['PStep'].modifiers['W'].vertex_group = 'G'
bpy.data.objects['PStep'].modifiers['W'].proximity_mode = 'GEOMETRY'
bpy.data.objects['PStep'].modifiers['W'].proximity_geometry = {'FACE'}
bpy.data.objects['PStep'].modifiers['W'].target = bpy.data.objects['T']
bpy.data.objects['PStep'].modifiers['W'].min_dist = 0.9
bpy.data.objects['PStep'].modifiers['W'].max_dist = 0.9
bpy.data.objects['PStep'].modifiers.new(name='Probe', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['PStep'].modifiers['Probe'].vertex_group = 'G'
bpy.data.objects['PStep'].modifiers['Probe'].use_add = True
bpy.data.objects['PStep'].modifiers['Probe'].add_threshold = 0.0
bpy.data.objects['PStep'].modifiers['Probe'].default_weight = 0.333
bpy.data.objects['PStep'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['PStep'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['PStep'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['PStep'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['PStep'].modifiers['Dz'].strength = 1.0
bpy.ops.mesh.primitive_grid_add(x_subdivisions=7, y_subdivisions=7, size=2)
bpy.context.object.name = 'PSmooth'
bpy.data.objects['PSmooth'].vertex_groups.new(name='G')
bpy.data.objects['PSmooth'].vertex_groups['G'].add([5, 35, 38], 0.1, 'REPLACE')
bpy.data.objects['PSmooth'].vertex_groups['G'].add([10, 31], 0.2, 'REPLACE')
bpy.data.objects['PSmooth'].vertex_groups['G'].add([0, 15, 22, 47, 52], 0.3, 'REPLACE')
bpy.data.objects['PSmooth'].vertex_groups['G'].add([13, 20, 50], 0.4, 'REPLACE')
bpy.data.objects['PSmooth'].vertex_groups['G'].add([6, 8, 9, 17, 24, 32, 41, 54, 63], 0.5, 'REPLACE')
bpy.data.objects['PSmooth'].vertex_groups['G'].add([1, 11, 29], 0.6, 'REPLACE')
bpy.data.objects['PSmooth'].vertex_groups['G'].add([12, 14, 26, 59, 60, 62], 0.7, 'REPLACE')
bpy.data.objects['PSmooth'].vertex_groups['G'].add([4, 21, 33, 43, 48, 56], 0.8, 'REPLACE')
bpy.data.objects['PSmooth'].vertex_groups['G'].add([7, 16, 34, 42, 61], 0.9, 'REPLACE')
bpy.data.objects['PSmooth'].vertex_groups['G'].add([18, 46, 53, 55, 58], 1.0, 'REPLACE')
bpy.data.objects['PSmooth'].modifiers.new(name='W', type='VERTEX_WEIGHT_PROXIMITY')
bpy.data.objects['PSmooth'].modifiers['W'].vertex_group = 'G'
bpy.data.objects['PSmooth'].modifiers['W'].proximity_mode = 'GEOMETRY'
bpy.data.objects['PSmooth'].modifiers['W'].proximity_geometry = {'EDGE'}
bpy.data.objects['PSmooth'].modifiers['W'].target = bpy.data.objects['T']
bpy.data.objects['PSmooth'].modifiers['W'].max_dist = 1.5
bpy.data.objects['PSmooth'].modifiers['W'].falloff_type = 'SMOOTH'
bpy.data.objects['PSmooth'].modifiers['W'].invert_falloff = True
bpy.data.objects['PSmooth'].modifiers.new(name='Probe', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['PSmooth'].modifiers['Probe'].vertex_group = 'G'
bpy.data.objects['PSmooth'].modifiers['Probe'].use_add = True
bpy.data.objects['PSmooth'].modifiers['Probe'].add_threshold = 0.0
bpy.data.objects['PSmooth'].modifiers['Probe'].default_weight = 0.333
bpy.data.objects['PSmooth'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['PSmooth'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['PSmooth'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['PSmooth'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['PSmooth'].modifiers['Dz'].strength = 1.0
bpy.ops.mesh.primitive_grid_add(x_subdivisions=7, y_subdivisions=7, size=2)
bpy.context.object.name = 'PNorm'
bpy.data.objects['PNorm'].vertex_groups.new(name='G')
bpy.data.objects['PNorm'].vertex_groups['G'].add([3, 7, 21, 38, 42], 0.1, 'REPLACE')
bpy.data.objects['PNorm'].vertex_groups['G'].add([11, 27, 34, 53], 0.2, 'REPLACE')
bpy.data.objects['PNorm'].vertex_groups['G'].add([50], 0.3, 'REPLACE')
bpy.data.objects['PNorm'].vertex_groups['G'].add([8, 49, 51, 54, 63], 0.4, 'REPLACE')
bpy.data.objects['PNorm'].vertex_groups['G'].add([31, 45, 47], 0.5, 'REPLACE')
bpy.data.objects['PNorm'].vertex_groups['G'].add([10, 12, 13, 15, 43, 52], 0.6, 'REPLACE')
bpy.data.objects['PNorm'].vertex_groups['G'].add([22, 32, 35, 36, 37], 0.7, 'REPLACE')
bpy.data.objects['PNorm'].vertex_groups['G'].add([6, 20, 26, 44, 58, 61], 0.8, 'REPLACE')
bpy.data.objects['PNorm'].vertex_groups['G'].add([0, 5, 9, 29, 41, 55], 0.9, 'REPLACE')
bpy.data.objects['PNorm'].vertex_groups['G'].add([4, 19, 25, 33, 46, 56], 1.0, 'REPLACE')
bpy.data.objects['PNorm'].modifiers.new(name='W', type='VERTEX_WEIGHT_PROXIMITY')
bpy.data.objects['PNorm'].modifiers['W'].vertex_group = 'G'
bpy.data.objects['PNorm'].modifiers['W'].proximity_mode = 'GEOMETRY'
bpy.data.objects['PNorm'].modifiers['W'].proximity_geometry = {'VERTEX'}
bpy.data.objects['PNorm'].modifiers['W'].target = bpy.data.objects['T']
bpy.data.objects['PNorm'].modifiers['W'].max_dist = 3.0
bpy.data.objects['PNorm'].modifiers['W'].normalize = True
bpy.data.objects['PNorm'].modifiers.new(name='Probe', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['PNorm'].modifiers['Probe'].vertex_group = 'G'
bpy.data.objects['PNorm'].modifiers['Probe'].use_add = True
bpy.data.objects['PNorm'].modifiers['Probe'].add_threshold = 0.0
bpy.data.objects['PNorm'].modifiers['Probe'].default_weight = 0.333
bpy.data.objects['PNorm'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['PNorm'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['PNorm'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['PNorm'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['PNorm'].modifiers['Dz'].strength = 1.0
bpy.ops.mesh.primitive_grid_add(x_subdivisions=7, y_subdivisions=7, size=2)
bpy.context.object.name = 'PMask'
bpy.data.objects['PMask'].vertex_groups.new(name='G')
bpy.data.objects['PMask'].vertex_groups['G'].add([3, 27, 32, 52, 56], 0.1, 'REPLACE')
bpy.data.objects['PMask'].vertex_groups['G'].add([0, 10, 20, 31, 34, 36, 38], 0.2, 'REPLACE')
bpy.data.objects['PMask'].vertex_groups['G'].add([2, 4, 18, 30, 33, 59], 0.3, 'REPLACE')
bpy.data.objects['PMask'].vertex_groups['G'].add([8, 13, 39, 40], 0.4, 'REPLACE')
bpy.data.objects['PMask'].vertex_groups['G'].add([7, 15, 16, 23, 28, 29, 42, 43, 45, 49, 60], 0.5, 'REPLACE')
bpy.data.objects['PMask'].vertex_groups['G'].add([12, 17, 51, 55, 57, 61, 62], 0.6, 'REPLACE')
bpy.data.objects['PMask'].vertex_groups['G'].add([5, 11, 26, 35, 50, 53], 0.8, 'REPLACE')
bpy.data.objects['PMask'].vertex_groups['G'].add([48], 0.9, 'REPLACE')
bpy.data.objects['PMask'].vertex_groups['G'].add([1, 6, 9, 14, 37, 46, 63], 1.0, 'REPLACE')
bpy.data.objects['PMask'].vertex_groups.new(name='M')
bpy.data.objects['PMask'].vertex_groups['M'].add([9, 50, 51, 53], 0.1, 'REPLACE')
bpy.data.objects['PMask'].vertex_groups['M'].add([6, 23, 24, 34, 38, 43, 44, 54, 56], 0.2, 'REPLACE')
bpy.data.objects['PMask'].vertex_groups['M'].add([15, 16, 41, 48, 59], 0.3, 'REPLACE')
bpy.data.objects['PMask'].vertex_groups['M'].add([3, 4, 12, 33, 35, 45, 60], 0.4, 'REPLACE')
bpy.data.objects['PMask'].vertex_groups['M'].add([18, 22, 29, 42, 57, 61], 0.5, 'REPLACE')
bpy.data.objects['PMask'].vertex_groups['M'].add([0, 2, 8, 26, 27, 32, 37, 46], 0.6, 'REPLACE')
bpy.data.objects['PMask'].vertex_groups['M'].add([7, 10, 14], 0.7, 'REPLACE')
bpy.data.objects['PMask'].vertex_groups['M'].add([5, 28, 30, 31, 49, 52, 63], 0.8, 'REPLACE')
bpy.data.objects['PMask'].vertex_groups['M'].add([21, 39, 40], 0.9, 'REPLACE')
bpy.data.objects['PMask'].vertex_groups['M'].add([1, 20, 58, 62], 1.0, 'REPLACE')
bpy.data.objects['PMask'].modifiers.new(name='W', type='VERTEX_WEIGHT_PROXIMITY')
bpy.data.objects['PMask'].modifiers['W'].vertex_group = 'G'
bpy.data.objects['PMask'].modifiers['W'].proximity_mode = 'GEOMETRY'
bpy.data.objects['PMask'].modifiers['W'].proximity_geometry = {'FACE'}
bpy.data.objects['PMask'].modifiers['W'].target = bpy.data.objects['T']
bpy.data.objects['PMask'].modifiers['W'].max_dist = 1.5
bpy.data.objects['PMask'].modifiers['W'].mask_constant = 0.5
bpy.data.objects['PMask'].modifiers['W'].mask_vertex_group = 'M'
bpy.data.objects['PMask'].modifiers.new(name='Probe', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['PMask'].modifiers['Probe'].vertex_group = 'G'
bpy.data.objects['PMask'].modifiers['Probe'].use_add = True
bpy.data.objects['PMask'].modifiers['Probe'].add_threshold = 0.0
bpy.data.objects['PMask'].modifiers['Probe'].default_weight = 0.333
bpy.data.objects['PMask'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['PMask'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['PMask'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['PMask'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['PMask'].modifiers['Dz'].strength = 1.0
bpy.ops.mesh.primitive_grid_add(x_subdivisions=7, y_subdivisions=7, size=2)
bpy.context.object.name = 'PNoTarget'
bpy.data.objects['PNoTarget'].vertex_groups.new(name='G')
bpy.data.objects['PNoTarget'].vertex_groups['G'].add([11, 20, 55], 0.1, 'REPLACE')
bpy.data.objects['PNoTarget'].vertex_groups['G'].add([2, 5, 6, 13, 14, 15, 35, 44, 50, 57, 60, 61], 0.2, 'REPLACE')
bpy.data.objects['PNoTarget'].vertex_groups['G'].add([8, 9, 25, 42], 0.3, 'REPLACE')
bpy.data.objects['PNoTarget'].vertex_groups['G'].add([17, 22, 47, 49], 0.4, 'REPLACE')
bpy.data.objects['PNoTarget'].vertex_groups['G'].add([4, 27, 39, 52, 63], 0.5, 'REPLACE')
bpy.data.objects['PNoTarget'].vertex_groups['G'].add([21, 30], 0.6, 'REPLACE')
bpy.data.objects['PNoTarget'].vertex_groups['G'].add([0, 18, 38, 43, 59], 0.7, 'REPLACE')
bpy.data.objects['PNoTarget'].vertex_groups['G'].add([19, 40, 51], 0.8, 'REPLACE')
bpy.data.objects['PNoTarget'].vertex_groups['G'].add([45, 53], 0.9, 'REPLACE')
bpy.data.objects['PNoTarget'].vertex_groups['G'].add([7, 12, 23, 24, 56], 1.0, 'REPLACE')
bpy.data.objects['PNoTarget'].modifiers.new(name='W', type='VERTEX_WEIGHT_PROXIMITY')
bpy.data.objects['PNoTarget'].modifiers['W'].vertex_group = 'G'
bpy.data.objects['PNoTarget'].modifiers['W'].proximity_mode = 'GEOMETRY'
bpy.data.objects['PNoTarget'].modifiers['W'].proximity_geometry = {'FACE'}
bpy.data.objects['PNoTarget'].modifiers.new(name='Probe', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['PNoTarget'].modifiers['Probe'].vertex_group = 'G'
bpy.data.objects['PNoTarget'].modifiers['Probe'].use_add = True
bpy.data.objects['PNoTarget'].modifiers['Probe'].add_threshold = 0.0
bpy.data.objects['PNoTarget'].modifiers['Probe'].default_weight = 0.333
bpy.data.objects['PNoTarget'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['PNoTarget'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['PNoTarget'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['PNoTarget'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['PNoTarget'].modifiers['Dz'].strength = 1.0
bpy.data.objects['PObject'].location = (0.4, -0.3, 0.0)
