# WP 8.3.13: VERTEX_WEIGHT_EDIT: every falloff_type (CURVE with the default curve, RANDOM
# seeded by the object name), invert_falloff, use_add / use_remove with their thresholds
# and default_weight, normalize, mask_constant and the mask vertex group (inverted), a
# missing group, a mesh without weights. A second VERTEX_WEIGHT_EDIT adds the vertices
# left outside G at 0.333 and DISPLACE shows the weights along Z.
# golden: startup=empty compare=evaluated:ELinear,evaluated:ECurve,evaluated:ESharp,evaluated:ESmooth,evaluated:ERoot,evaluated:ESphere,evaluated:ERandom,evaluated:EStep,evaluated:EInvSharp,evaluated:EAdd,evaluated:EAddLow,evaluated:ERemove,evaluated:ENormalize,evaluated:EInfluence,evaluated:EMaskGroup,evaluated:EMaskInv,evaluated:ENoGroup,evaluated:EEmpty tier=T2
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=7, y_subdivisions=7, size=2)
bpy.context.object.name = 'ELinear'
bpy.data.objects['ELinear'].vertex_groups.new(name='G')
bpy.data.objects['ELinear'].vertex_groups['G'].add([1, 6, 19, 20, 46, 49], 0.1, 'REPLACE')
bpy.data.objects['ELinear'].vertex_groups['G'].add([9, 56, 58, 61], 0.2, 'REPLACE')
bpy.data.objects['ELinear'].vertex_groups['G'].add([21, 34], 0.3, 'REPLACE')
bpy.data.objects['ELinear'].vertex_groups['G'].add([45, 52, 55], 0.4, 'REPLACE')
bpy.data.objects['ELinear'].vertex_groups['G'].add([15, 26, 50], 0.5, 'REPLACE')
bpy.data.objects['ELinear'].vertex_groups['G'].add([29, 33, 36, 43, 47, 54], 0.6, 'REPLACE')
bpy.data.objects['ELinear'].vertex_groups['G'].add([0, 11, 12, 60, 62, 63], 0.7, 'REPLACE')
bpy.data.objects['ELinear'].vertex_groups['G'].add([4, 14, 41, 48], 0.8, 'REPLACE')
bpy.data.objects['ELinear'].vertex_groups['G'].add([3, 13, 16, 32, 59], 0.9, 'REPLACE')
bpy.data.objects['ELinear'].vertex_groups['G'].add([8, 30, 35, 57], 1.0, 'REPLACE')
bpy.data.objects['ELinear'].modifiers.new(name='W', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['ELinear'].modifiers['W'].vertex_group = 'G'
bpy.data.objects['ELinear'].modifiers.new(name='Probe', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['ELinear'].modifiers['Probe'].vertex_group = 'G'
bpy.data.objects['ELinear'].modifiers['Probe'].use_add = True
bpy.data.objects['ELinear'].modifiers['Probe'].add_threshold = 0.0
bpy.data.objects['ELinear'].modifiers['Probe'].default_weight = 0.333
bpy.data.objects['ELinear'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['ELinear'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['ELinear'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['ELinear'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['ELinear'].modifiers['Dz'].strength = 1.0
bpy.ops.mesh.primitive_grid_add(x_subdivisions=7, y_subdivisions=7, size=2)
bpy.context.object.name = 'ECurve'
bpy.data.objects['ECurve'].vertex_groups.new(name='G')
bpy.data.objects['ECurve'].vertex_groups['G'].add([9, 11, 33], 0.1, 'REPLACE')
bpy.data.objects['ECurve'].vertex_groups['G'].add([10, 25, 26, 45, 49], 0.2, 'REPLACE')
bpy.data.objects['ECurve'].vertex_groups['G'].add([1, 2, 14, 52, 55, 57], 0.4, 'REPLACE')
bpy.data.objects['ECurve'].vertex_groups['G'].add([18, 24, 28], 0.5, 'REPLACE')
bpy.data.objects['ECurve'].vertex_groups['G'].add([12, 15, 22, 53, 56], 0.6, 'REPLACE')
bpy.data.objects['ECurve'].vertex_groups['G'].add([3, 21, 31, 34, 35, 39, 46, 58, 61, 62], 0.7, 'REPLACE')
bpy.data.objects['ECurve'].vertex_groups['G'].add([13, 30, 36, 48, 50, 51], 0.8, 'REPLACE')
bpy.data.objects['ECurve'].vertex_groups['G'].add([7, 44], 0.9, 'REPLACE')
bpy.data.objects['ECurve'].vertex_groups['G'].add([17, 43, 63], 1.0, 'REPLACE')
bpy.data.objects['ECurve'].modifiers.new(name='W', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['ECurve'].modifiers['W'].vertex_group = 'G'
bpy.data.objects['ECurve'].modifiers['W'].falloff_type = 'CURVE'
bpy.data.objects['ECurve'].modifiers.new(name='Probe', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['ECurve'].modifiers['Probe'].vertex_group = 'G'
bpy.data.objects['ECurve'].modifiers['Probe'].use_add = True
bpy.data.objects['ECurve'].modifiers['Probe'].add_threshold = 0.0
bpy.data.objects['ECurve'].modifiers['Probe'].default_weight = 0.333
bpy.data.objects['ECurve'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['ECurve'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['ECurve'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['ECurve'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['ECurve'].modifiers['Dz'].strength = 1.0
bpy.ops.mesh.primitive_grid_add(x_subdivisions=7, y_subdivisions=7, size=2)
bpy.context.object.name = 'ESharp'
bpy.data.objects['ESharp'].vertex_groups.new(name='G')
bpy.data.objects['ESharp'].vertex_groups['G'].add([3, 6, 13, 31, 34, 36, 38, 48], 0.1, 'REPLACE')
bpy.data.objects['ESharp'].vertex_groups['G'].add([51, 63], 0.2, 'REPLACE')
bpy.data.objects['ESharp'].vertex_groups['G'].add([0, 9, 10, 21, 23, 27, 54, 58], 0.3, 'REPLACE')
bpy.data.objects['ESharp'].vertex_groups['G'].add([35, 49, 62], 0.4, 'REPLACE')
bpy.data.objects['ESharp'].vertex_groups['G'].add([11, 12, 52, 60], 0.5, 'REPLACE')
bpy.data.objects['ESharp'].vertex_groups['G'].add([17, 42, 47], 0.6, 'REPLACE')
bpy.data.objects['ESharp'].vertex_groups['G'].add([33, 37], 0.7, 'REPLACE')
bpy.data.objects['ESharp'].vertex_groups['G'].add([8, 16, 19, 32], 0.8, 'REPLACE')
bpy.data.objects['ESharp'].vertex_groups['G'].add([14, 25, 28, 40, 53], 0.9, 'REPLACE')
bpy.data.objects['ESharp'].vertex_groups['G'].add([39, 46, 50], 1.0, 'REPLACE')
bpy.data.objects['ESharp'].modifiers.new(name='W', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['ESharp'].modifiers['W'].vertex_group = 'G'
bpy.data.objects['ESharp'].modifiers['W'].falloff_type = 'SHARP'
bpy.data.objects['ESharp'].modifiers.new(name='Probe', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['ESharp'].modifiers['Probe'].vertex_group = 'G'
bpy.data.objects['ESharp'].modifiers['Probe'].use_add = True
bpy.data.objects['ESharp'].modifiers['Probe'].add_threshold = 0.0
bpy.data.objects['ESharp'].modifiers['Probe'].default_weight = 0.333
bpy.data.objects['ESharp'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['ESharp'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['ESharp'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['ESharp'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['ESharp'].modifiers['Dz'].strength = 1.0
bpy.ops.mesh.primitive_grid_add(x_subdivisions=7, y_subdivisions=7, size=2)
bpy.context.object.name = 'ESmooth'
bpy.data.objects['ESmooth'].vertex_groups.new(name='G')
bpy.data.objects['ESmooth'].vertex_groups['G'].add([5, 18, 20, 42], 0.1, 'REPLACE')
bpy.data.objects['ESmooth'].vertex_groups['G'].add([15, 29, 52], 0.2, 'REPLACE')
bpy.data.objects['ESmooth'].vertex_groups['G'].add([3, 40, 57, 61], 0.3, 'REPLACE')
bpy.data.objects['ESmooth'].vertex_groups['G'].add([10, 43], 0.4, 'REPLACE')
bpy.data.objects['ESmooth'].vertex_groups['G'].add([2, 19, 22, 28, 44, 53, 63], 0.5, 'REPLACE')
bpy.data.objects['ESmooth'].vertex_groups['G'].add([0, 9, 24], 0.6, 'REPLACE')
bpy.data.objects['ESmooth'].vertex_groups['G'].add([1, 8, 14, 23, 30, 33, 34, 36, 49, 51, 56], 0.7, 'REPLACE')
bpy.data.objects['ESmooth'].vertex_groups['G'].add([11, 35, 47], 0.8, 'REPLACE')
bpy.data.objects['ESmooth'].vertex_groups['G'].add([27, 59], 0.9, 'REPLACE')
bpy.data.objects['ESmooth'].vertex_groups['G'].add([7, 13, 17, 21, 32, 41, 46], 1.0, 'REPLACE')
bpy.data.objects['ESmooth'].modifiers.new(name='W', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['ESmooth'].modifiers['W'].vertex_group = 'G'
bpy.data.objects['ESmooth'].modifiers['W'].falloff_type = 'SMOOTH'
bpy.data.objects['ESmooth'].modifiers.new(name='Probe', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['ESmooth'].modifiers['Probe'].vertex_group = 'G'
bpy.data.objects['ESmooth'].modifiers['Probe'].use_add = True
bpy.data.objects['ESmooth'].modifiers['Probe'].add_threshold = 0.0
bpy.data.objects['ESmooth'].modifiers['Probe'].default_weight = 0.333
bpy.data.objects['ESmooth'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['ESmooth'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['ESmooth'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['ESmooth'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['ESmooth'].modifiers['Dz'].strength = 1.0
bpy.ops.mesh.primitive_grid_add(x_subdivisions=7, y_subdivisions=7, size=2)
bpy.context.object.name = 'ERoot'
bpy.data.objects['ERoot'].vertex_groups.new(name='G')
bpy.data.objects['ERoot'].vertex_groups['G'].add([0, 11, 28, 33], 0.1, 'REPLACE')
bpy.data.objects['ERoot'].vertex_groups['G'].add([8, 35], 0.2, 'REPLACE')
bpy.data.objects['ERoot'].vertex_groups['G'].add([19, 21, 31, 32, 38, 46], 0.3, 'REPLACE')
bpy.data.objects['ERoot'].vertex_groups['G'].add([18, 40, 41, 43], 0.4, 'REPLACE')
bpy.data.objects['ERoot'].vertex_groups['G'].add([17, 30, 55], 0.5, 'REPLACE')
bpy.data.objects['ERoot'].vertex_groups['G'].add([1, 6, 16, 24, 25, 53], 0.6, 'REPLACE')
bpy.data.objects['ERoot'].vertex_groups['G'].add([2, 3, 47, 49], 0.7, 'REPLACE')
bpy.data.objects['ERoot'].vertex_groups['G'].add([36, 45], 0.8, 'REPLACE')
bpy.data.objects['ERoot'].vertex_groups['G'].add([9, 10, 14, 20, 50, 59], 0.9, 'REPLACE')
bpy.data.objects['ERoot'].vertex_groups['G'].add([13, 15, 42], 1.0, 'REPLACE')
bpy.data.objects['ERoot'].modifiers.new(name='W', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['ERoot'].modifiers['W'].vertex_group = 'G'
bpy.data.objects['ERoot'].modifiers['W'].falloff_type = 'ROOT'
bpy.data.objects['ERoot'].modifiers.new(name='Probe', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['ERoot'].modifiers['Probe'].vertex_group = 'G'
bpy.data.objects['ERoot'].modifiers['Probe'].use_add = True
bpy.data.objects['ERoot'].modifiers['Probe'].add_threshold = 0.0
bpy.data.objects['ERoot'].modifiers['Probe'].default_weight = 0.333
bpy.data.objects['ERoot'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['ERoot'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['ERoot'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['ERoot'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['ERoot'].modifiers['Dz'].strength = 1.0
bpy.ops.mesh.primitive_grid_add(x_subdivisions=7, y_subdivisions=7, size=2)
bpy.context.object.name = 'ESphere'
bpy.data.objects['ESphere'].vertex_groups.new(name='G')
bpy.data.objects['ESphere'].vertex_groups['G'].add([3, 16, 49], 0.1, 'REPLACE')
bpy.data.objects['ESphere'].vertex_groups['G'].add([18, 26, 27, 33, 35, 44, 45, 48], 0.2, 'REPLACE')
bpy.data.objects['ESphere'].vertex_groups['G'].add([7, 9, 19, 22, 29, 34, 43, 47, 61], 0.3, 'REPLACE')
bpy.data.objects['ESphere'].vertex_groups['G'].add([0, 20, 28, 53], 0.4, 'REPLACE')
bpy.data.objects['ESphere'].vertex_groups['G'].add([1, 12, 40], 0.5, 'REPLACE')
bpy.data.objects['ESphere'].vertex_groups['G'].add([56], 0.6, 'REPLACE')
bpy.data.objects['ESphere'].vertex_groups['G'].add([2, 5, 8, 39, 50, 52], 0.7, 'REPLACE')
bpy.data.objects['ESphere'].vertex_groups['G'].add([4, 25, 30, 42, 57, 60], 0.8, 'REPLACE')
bpy.data.objects['ESphere'].vertex_groups['G'].add([23, 58], 0.9, 'REPLACE')
bpy.data.objects['ESphere'].vertex_groups['G'].add([11, 24, 31, 36], 1.0, 'REPLACE')
bpy.data.objects['ESphere'].modifiers.new(name='W', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['ESphere'].modifiers['W'].vertex_group = 'G'
bpy.data.objects['ESphere'].modifiers['W'].falloff_type = 'ICON_SPHERECURVE'
bpy.data.objects['ESphere'].modifiers.new(name='Probe', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['ESphere'].modifiers['Probe'].vertex_group = 'G'
bpy.data.objects['ESphere'].modifiers['Probe'].use_add = True
bpy.data.objects['ESphere'].modifiers['Probe'].add_threshold = 0.0
bpy.data.objects['ESphere'].modifiers['Probe'].default_weight = 0.333
bpy.data.objects['ESphere'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['ESphere'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['ESphere'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['ESphere'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['ESphere'].modifiers['Dz'].strength = 1.0
bpy.ops.mesh.primitive_grid_add(x_subdivisions=7, y_subdivisions=7, size=2)
bpy.context.object.name = 'ERandom'
bpy.data.objects['ERandom'].vertex_groups.new(name='G')
bpy.data.objects['ERandom'].vertex_groups['G'].add([18, 49, 54], 0.1, 'REPLACE')
bpy.data.objects['ERandom'].vertex_groups['G'].add([12, 31, 40, 51, 62], 0.2, 'REPLACE')
bpy.data.objects['ERandom'].vertex_groups['G'].add([9, 27, 34, 44, 50, 58], 0.3, 'REPLACE')
bpy.data.objects['ERandom'].vertex_groups['G'].add([1, 3, 8, 35], 0.4, 'REPLACE')
bpy.data.objects['ERandom'].vertex_groups['G'].add([0, 22, 32, 42, 60], 0.5, 'REPLACE')
bpy.data.objects['ERandom'].vertex_groups['G'].add([2, 4, 17, 21, 59, 61], 0.6, 'REPLACE')
bpy.data.objects['ERandom'].vertex_groups['G'].add([10, 11], 0.7, 'REPLACE')
bpy.data.objects['ERandom'].vertex_groups['G'].add([20, 33, 53], 0.8, 'REPLACE')
bpy.data.objects['ERandom'].vertex_groups['G'].add([16, 24, 39, 45, 46, 55], 0.9, 'REPLACE')
bpy.data.objects['ERandom'].vertex_groups['G'].add([25, 29, 37], 1.0, 'REPLACE')
bpy.data.objects['ERandom'].modifiers.new(name='W', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['ERandom'].modifiers['W'].vertex_group = 'G'
bpy.data.objects['ERandom'].modifiers['W'].falloff_type = 'RANDOM'
bpy.data.objects['ERandom'].modifiers.new(name='Probe', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['ERandom'].modifiers['Probe'].vertex_group = 'G'
bpy.data.objects['ERandom'].modifiers['Probe'].use_add = True
bpy.data.objects['ERandom'].modifiers['Probe'].add_threshold = 0.0
bpy.data.objects['ERandom'].modifiers['Probe'].default_weight = 0.333
bpy.data.objects['ERandom'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['ERandom'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['ERandom'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['ERandom'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['ERandom'].modifiers['Dz'].strength = 1.0
bpy.ops.mesh.primitive_grid_add(x_subdivisions=7, y_subdivisions=7, size=2)
bpy.context.object.name = 'EStep'
bpy.data.objects['EStep'].vertex_groups.new(name='G')
bpy.data.objects['EStep'].vertex_groups['G'].add([11, 24, 49], 0.1, 'REPLACE')
bpy.data.objects['EStep'].vertex_groups['G'].add([10, 26, 35, 50], 0.2, 'REPLACE')
bpy.data.objects['EStep'].vertex_groups['G'].add([16, 21, 31, 44, 53, 55], 0.3, 'REPLACE')
bpy.data.objects['EStep'].vertex_groups['G'].add([3, 17, 27, 29, 34, 41, 61], 0.4, 'REPLACE')
bpy.data.objects['EStep'].vertex_groups['G'].add([14, 30], 0.5, 'REPLACE')
bpy.data.objects['EStep'].vertex_groups['G'].add([6, 39], 0.6, 'REPLACE')
bpy.data.objects['EStep'].vertex_groups['G'].add([4, 18, 28, 52, 62], 0.7, 'REPLACE')
bpy.data.objects['EStep'].vertex_groups['G'].add([15, 19, 37, 40, 48], 0.8, 'REPLACE')
bpy.data.objects['EStep'].vertex_groups['G'].add([0, 8, 20], 0.9, 'REPLACE')
bpy.data.objects['EStep'].vertex_groups['G'].add([25, 51, 58, 60], 1.0, 'REPLACE')
bpy.data.objects['EStep'].modifiers.new(name='W', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['EStep'].modifiers['W'].vertex_group = 'G'
bpy.data.objects['EStep'].modifiers['W'].falloff_type = 'STEP'
bpy.data.objects['EStep'].modifiers.new(name='Probe', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['EStep'].modifiers['Probe'].vertex_group = 'G'
bpy.data.objects['EStep'].modifiers['Probe'].use_add = True
bpy.data.objects['EStep'].modifiers['Probe'].add_threshold = 0.0
bpy.data.objects['EStep'].modifiers['Probe'].default_weight = 0.333
bpy.data.objects['EStep'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['EStep'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['EStep'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['EStep'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['EStep'].modifiers['Dz'].strength = 1.0
bpy.ops.mesh.primitive_grid_add(x_subdivisions=7, y_subdivisions=7, size=2)
bpy.context.object.name = 'EInvSharp'
bpy.data.objects['EInvSharp'].vertex_groups.new(name='G')
bpy.data.objects['EInvSharp'].vertex_groups['G'].add([1, 5, 27, 39, 40, 50, 58], 0.1, 'REPLACE')
bpy.data.objects['EInvSharp'].vertex_groups['G'].add([7, 11, 17, 34, 53], 0.2, 'REPLACE')
bpy.data.objects['EInvSharp'].vertex_groups['G'].add([19, 32, 49], 0.3, 'REPLACE')
bpy.data.objects['EInvSharp'].vertex_groups['G'].add([8, 59], 0.4, 'REPLACE')
bpy.data.objects['EInvSharp'].vertex_groups['G'].add([0, 22, 33, 47, 52, 55, 62], 0.5, 'REPLACE')
bpy.data.objects['EInvSharp'].vertex_groups['G'].add([13, 14, 26, 31, 36, 46, 51, 57], 0.6, 'REPLACE')
bpy.data.objects['EInvSharp'].vertex_groups['G'].add([12, 23, 29, 38, 60], 0.7, 'REPLACE')
bpy.data.objects['EInvSharp'].vertex_groups['G'].add([56], 0.8, 'REPLACE')
bpy.data.objects['EInvSharp'].vertex_groups['G'].add([6, 9, 43, 54, 61], 0.9, 'REPLACE')
bpy.data.objects['EInvSharp'].vertex_groups['G'].add([25, 37, 42, 48, 63], 1.0, 'REPLACE')
bpy.data.objects['EInvSharp'].modifiers.new(name='W', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['EInvSharp'].modifiers['W'].vertex_group = 'G'
bpy.data.objects['EInvSharp'].modifiers['W'].falloff_type = 'SHARP'
bpy.data.objects['EInvSharp'].modifiers['W'].invert_falloff = True
bpy.data.objects['EInvSharp'].modifiers.new(name='Probe', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['EInvSharp'].modifiers['Probe'].vertex_group = 'G'
bpy.data.objects['EInvSharp'].modifiers['Probe'].use_add = True
bpy.data.objects['EInvSharp'].modifiers['Probe'].add_threshold = 0.0
bpy.data.objects['EInvSharp'].modifiers['Probe'].default_weight = 0.333
bpy.data.objects['EInvSharp'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['EInvSharp'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['EInvSharp'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['EInvSharp'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['EInvSharp'].modifiers['Dz'].strength = 1.0
bpy.ops.mesh.primitive_grid_add(x_subdivisions=7, y_subdivisions=7, size=2)
bpy.context.object.name = 'EAdd'
bpy.data.objects['EAdd'].vertex_groups.new(name='G')
bpy.data.objects['EAdd'].vertex_groups['G'].add([35, 40, 53, 59], 0.1, 'REPLACE')
bpy.data.objects['EAdd'].vertex_groups['G'].add([1, 14, 21], 0.2, 'REPLACE')
bpy.data.objects['EAdd'].vertex_groups['G'].add([27, 30, 47, 54, 61, 63], 0.3, 'REPLACE')
bpy.data.objects['EAdd'].vertex_groups['G'].add([8, 38, 55], 0.4, 'REPLACE')
bpy.data.objects['EAdd'].vertex_groups['G'].add([26, 28, 31, 33, 36, 37], 0.5, 'REPLACE')
bpy.data.objects['EAdd'].vertex_groups['G'].add([4, 41, 45, 49], 0.6, 'REPLACE')
bpy.data.objects['EAdd'].vertex_groups['G'].add([51, 56], 0.7, 'REPLACE')
bpy.data.objects['EAdd'].vertex_groups['G'].add([7, 58], 0.8, 'REPLACE')
bpy.data.objects['EAdd'].vertex_groups['G'].add([3, 10, 12, 16, 19, 22, 23, 29, 39], 0.9, 'REPLACE')
bpy.data.objects['EAdd'].vertex_groups['G'].add([2, 5, 32], 1.0, 'REPLACE')
bpy.data.objects['EAdd'].modifiers.new(name='W', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['EAdd'].modifiers['W'].vertex_group = 'G'
bpy.data.objects['EAdd'].modifiers['W'].use_add = True
bpy.data.objects['EAdd'].modifiers['W'].add_threshold = 0.3
bpy.data.objects['EAdd'].modifiers['W'].default_weight = 0.4
bpy.data.objects['EAdd'].modifiers.new(name='Probe', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['EAdd'].modifiers['Probe'].vertex_group = 'G'
bpy.data.objects['EAdd'].modifiers['Probe'].use_add = True
bpy.data.objects['EAdd'].modifiers['Probe'].add_threshold = 0.0
bpy.data.objects['EAdd'].modifiers['Probe'].default_weight = 0.333
bpy.data.objects['EAdd'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['EAdd'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['EAdd'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['EAdd'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['EAdd'].modifiers['Dz'].strength = 1.0
bpy.ops.mesh.primitive_grid_add(x_subdivisions=7, y_subdivisions=7, size=2)
bpy.context.object.name = 'EAddLow'
bpy.data.objects['EAddLow'].vertex_groups.new(name='G')
bpy.data.objects['EAddLow'].vertex_groups['G'].add([30, 32], 0.1, 'REPLACE')
bpy.data.objects['EAddLow'].vertex_groups['G'].add([8, 33, 52, 62, 63], 0.2, 'REPLACE')
bpy.data.objects['EAddLow'].vertex_groups['G'].add([1, 19, 26, 37, 39], 0.3, 'REPLACE')
bpy.data.objects['EAddLow'].vertex_groups['G'].add([10, 22, 38, 42], 0.4, 'REPLACE')
bpy.data.objects['EAddLow'].vertex_groups['G'].add([0, 6, 11, 18, 27, 57, 60], 0.5, 'REPLACE')
bpy.data.objects['EAddLow'].vertex_groups['G'].add([17, 31, 34, 43, 46, 48, 50, 54, 61], 0.6, 'REPLACE')
bpy.data.objects['EAddLow'].vertex_groups['G'].add([5, 9, 20], 0.7, 'REPLACE')
bpy.data.objects['EAddLow'].vertex_groups['G'].add([3, 24, 41, 44], 0.8, 'REPLACE')
bpy.data.objects['EAddLow'].vertex_groups['G'].add([4, 15, 36, 49, 53, 58], 0.9, 'REPLACE')
bpy.data.objects['EAddLow'].vertex_groups['G'].add([13, 40, 55], 1.0, 'REPLACE')
bpy.data.objects['EAddLow'].modifiers.new(name='W', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['EAddLow'].modifiers['W'].vertex_group = 'G'
bpy.data.objects['EAddLow'].modifiers['W'].use_add = True
bpy.data.objects['EAddLow'].modifiers['W'].add_threshold = 0.5
bpy.data.objects['EAddLow'].modifiers['W'].default_weight = 0.4
bpy.data.objects['EAddLow'].modifiers.new(name='Probe', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['EAddLow'].modifiers['Probe'].vertex_group = 'G'
bpy.data.objects['EAddLow'].modifiers['Probe'].use_add = True
bpy.data.objects['EAddLow'].modifiers['Probe'].add_threshold = 0.0
bpy.data.objects['EAddLow'].modifiers['Probe'].default_weight = 0.333
bpy.data.objects['EAddLow'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['EAddLow'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['EAddLow'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['EAddLow'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['EAddLow'].modifiers['Dz'].strength = 1.0
bpy.ops.mesh.primitive_grid_add(x_subdivisions=7, y_subdivisions=7, size=2)
bpy.context.object.name = 'ERemove'
bpy.data.objects['ERemove'].vertex_groups.new(name='G')
bpy.data.objects['ERemove'].vertex_groups['G'].add([3, 5, 22, 38], 0.1, 'REPLACE')
bpy.data.objects['ERemove'].vertex_groups['G'].add([4, 13, 25, 31, 58, 59, 62], 0.2, 'REPLACE')
bpy.data.objects['ERemove'].vertex_groups['G'].add([29, 32, 39, 49], 0.3, 'REPLACE')
bpy.data.objects['ERemove'].vertex_groups['G'].add([17, 18, 20, 21, 27, 28, 37, 40, 45], 0.4, 'REPLACE')
bpy.data.objects['ERemove'].vertex_groups['G'].add([14], 0.5, 'REPLACE')
bpy.data.objects['ERemove'].vertex_groups['G'].add([11, 15, 33, 42, 56, 60], 0.6, 'REPLACE')
bpy.data.objects['ERemove'].vertex_groups['G'].add([1, 8, 9, 61], 0.7, 'REPLACE')
bpy.data.objects['ERemove'].vertex_groups['G'].add([2, 7, 34, 41, 51, 52, 53], 0.8, 'REPLACE')
bpy.data.objects['ERemove'].vertex_groups['G'].add([43, 57, 63], 0.9, 'REPLACE')
bpy.data.objects['ERemove'].vertex_groups['G'].add([6, 26, 50, 54, 55], 1.0, 'REPLACE')
bpy.data.objects['ERemove'].modifiers.new(name='W', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['ERemove'].modifiers['W'].vertex_group = 'G'
bpy.data.objects['ERemove'].modifiers['W'].use_remove = True
bpy.data.objects['ERemove'].modifiers['W'].remove_threshold = 0.45
bpy.data.objects['ERemove'].modifiers.new(name='Probe', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['ERemove'].modifiers['Probe'].vertex_group = 'G'
bpy.data.objects['ERemove'].modifiers['Probe'].use_add = True
bpy.data.objects['ERemove'].modifiers['Probe'].add_threshold = 0.0
bpy.data.objects['ERemove'].modifiers['Probe'].default_weight = 0.333
bpy.data.objects['ERemove'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['ERemove'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['ERemove'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['ERemove'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['ERemove'].modifiers['Dz'].strength = 1.0
bpy.ops.mesh.primitive_grid_add(x_subdivisions=7, y_subdivisions=7, size=2)
bpy.context.object.name = 'ENormalize'
bpy.data.objects['ENormalize'].vertex_groups.new(name='G')
bpy.data.objects['ENormalize'].vertex_groups['G'].add([1, 27, 30], 0.1, 'REPLACE')
bpy.data.objects['ENormalize'].vertex_groups['G'].add([2, 10, 11, 48, 53, 54, 55, 61], 0.2, 'REPLACE')
bpy.data.objects['ENormalize'].vertex_groups['G'].add([5, 21, 23, 28, 38, 49], 0.3, 'REPLACE')
bpy.data.objects['ENormalize'].vertex_groups['G'].add([9, 12, 16, 20, 41, 45, 46, 52], 0.4, 'REPLACE')
bpy.data.objects['ENormalize'].vertex_groups['G'].add([0, 22, 26], 0.5, 'REPLACE')
bpy.data.objects['ENormalize'].vertex_groups['G'].add([4, 7, 15, 17, 57], 0.6, 'REPLACE')
bpy.data.objects['ENormalize'].vertex_groups['G'].add([13, 32, 34, 59], 0.7, 'REPLACE')
bpy.data.objects['ENormalize'].vertex_groups['G'].add([24, 37, 50, 63], 0.8, 'REPLACE')
bpy.data.objects['ENormalize'].vertex_groups['G'].add([3, 14, 29, 44, 60], 0.9, 'REPLACE')
bpy.data.objects['ENormalize'].vertex_groups['G'].add([6, 36, 42, 47, 62], 1.0, 'REPLACE')
bpy.data.objects['ENormalize'].modifiers.new(name='W', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['ENormalize'].modifiers['W'].vertex_group = 'G'
bpy.data.objects['ENormalize'].modifiers['W'].normalize = True
bpy.data.objects['ENormalize'].modifiers['W'].falloff_type = 'SHARP'
bpy.data.objects['ENormalize'].modifiers.new(name='Probe', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['ENormalize'].modifiers['Probe'].vertex_group = 'G'
bpy.data.objects['ENormalize'].modifiers['Probe'].use_add = True
bpy.data.objects['ENormalize'].modifiers['Probe'].add_threshold = 0.0
bpy.data.objects['ENormalize'].modifiers['Probe'].default_weight = 0.333
bpy.data.objects['ENormalize'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['ENormalize'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['ENormalize'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['ENormalize'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['ENormalize'].modifiers['Dz'].strength = 1.0
bpy.ops.mesh.primitive_grid_add(x_subdivisions=7, y_subdivisions=7, size=2)
bpy.context.object.name = 'EInfluence'
bpy.data.objects['EInfluence'].vertex_groups.new(name='G')
bpy.data.objects['EInfluence'].vertex_groups['G'].add([10, 39, 50, 59, 63], 0.1, 'REPLACE')
bpy.data.objects['EInfluence'].vertex_groups['G'].add([0, 1, 37, 52], 0.2, 'REPLACE')
bpy.data.objects['EInfluence'].vertex_groups['G'].add([5, 28, 51], 0.3, 'REPLACE')
bpy.data.objects['EInfluence'].vertex_groups['G'].add([8, 17, 24, 25, 30, 32, 44, 47], 0.4, 'REPLACE')
bpy.data.objects['EInfluence'].vertex_groups['G'].add([20, 40, 46, 55, 56, 58, 62], 0.5, 'REPLACE')
bpy.data.objects['EInfluence'].vertex_groups['G'].add([15, 53], 0.6, 'REPLACE')
bpy.data.objects['EInfluence'].vertex_groups['G'].add([22, 31, 43, 45], 0.7, 'REPLACE')
bpy.data.objects['EInfluence'].vertex_groups['G'].add([11], 0.8, 'REPLACE')
bpy.data.objects['EInfluence'].vertex_groups['G'].add([34, 35, 48, 57], 0.9, 'REPLACE')
bpy.data.objects['EInfluence'].vertex_groups['G'].add([2, 4, 6, 9, 14, 19, 38, 42, 61], 1.0, 'REPLACE')
bpy.data.objects['EInfluence'].modifiers.new(name='W', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['EInfluence'].modifiers['W'].vertex_group = 'G'
bpy.data.objects['EInfluence'].modifiers['W'].falloff_type = 'STEP'
bpy.data.objects['EInfluence'].modifiers['W'].mask_constant = 0.4
bpy.data.objects['EInfluence'].modifiers.new(name='Probe', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['EInfluence'].modifiers['Probe'].vertex_group = 'G'
bpy.data.objects['EInfluence'].modifiers['Probe'].use_add = True
bpy.data.objects['EInfluence'].modifiers['Probe'].add_threshold = 0.0
bpy.data.objects['EInfluence'].modifiers['Probe'].default_weight = 0.333
bpy.data.objects['EInfluence'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['EInfluence'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['EInfluence'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['EInfluence'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['EInfluence'].modifiers['Dz'].strength = 1.0
bpy.ops.mesh.primitive_grid_add(x_subdivisions=7, y_subdivisions=7, size=2)
bpy.context.object.name = 'EMaskGroup'
bpy.data.objects['EMaskGroup'].vertex_groups.new(name='G')
bpy.data.objects['EMaskGroup'].vertex_groups['G'].add([0, 21, 26, 60], 0.1, 'REPLACE')
bpy.data.objects['EMaskGroup'].vertex_groups['G'].add([8, 10, 14, 32, 36, 37, 47, 52], 0.2, 'REPLACE')
bpy.data.objects['EMaskGroup'].vertex_groups['G'].add([16, 18, 19, 20, 53], 0.3, 'REPLACE')
bpy.data.objects['EMaskGroup'].vertex_groups['G'].add([23, 29, 59], 0.5, 'REPLACE')
bpy.data.objects['EMaskGroup'].vertex_groups['G'].add([3, 5, 9, 13, 28], 0.6, 'REPLACE')
bpy.data.objects['EMaskGroup'].vertex_groups['G'].add([41, 46], 0.7, 'REPLACE')
bpy.data.objects['EMaskGroup'].vertex_groups['G'].add([1, 30, 38, 39, 43, 57, 58], 0.8, 'REPLACE')
bpy.data.objects['EMaskGroup'].vertex_groups['G'].add([2, 25], 0.9, 'REPLACE')
bpy.data.objects['EMaskGroup'].vertex_groups['G'].add([11, 44, 45, 54, 55, 63], 1.0, 'REPLACE')
bpy.data.objects['EMaskGroup'].vertex_groups.new(name='M')
bpy.data.objects['EMaskGroup'].vertex_groups['M'].add([4, 7, 8, 27, 39, 43, 47, 62], 0.1, 'REPLACE')
bpy.data.objects['EMaskGroup'].vertex_groups['M'].add([23, 35, 48, 51], 0.2, 'REPLACE')
bpy.data.objects['EMaskGroup'].vertex_groups['M'].add([24, 26, 30, 40, 59, 63], 0.3, 'REPLACE')
bpy.data.objects['EMaskGroup'].vertex_groups['M'].add([11, 16, 46, 50, 52, 58], 0.4, 'REPLACE')
bpy.data.objects['EMaskGroup'].vertex_groups['M'].add([14, 38, 45, 49, 56], 0.5, 'REPLACE')
bpy.data.objects['EMaskGroup'].vertex_groups['M'].add([18, 29, 32, 42, 55], 0.6, 'REPLACE')
bpy.data.objects['EMaskGroup'].vertex_groups['M'].add([15, 19, 22, 28, 33], 0.7, 'REPLACE')
bpy.data.objects['EMaskGroup'].vertex_groups['M'].add([13, 21, 25, 37], 0.8, 'REPLACE')
bpy.data.objects['EMaskGroup'].vertex_groups['M'].add([1, 2, 3, 10, 17, 31, 34, 61], 0.9, 'REPLACE')
bpy.data.objects['EMaskGroup'].vertex_groups['M'].add([5, 6, 12, 20, 54, 57], 1.0, 'REPLACE')
bpy.data.objects['EMaskGroup'].modifiers.new(name='W', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['EMaskGroup'].modifiers['W'].vertex_group = 'G'
bpy.data.objects['EMaskGroup'].modifiers['W'].falloff_type = 'SHARP'
bpy.data.objects['EMaskGroup'].modifiers['W'].invert_falloff = True
bpy.data.objects['EMaskGroup'].modifiers['W'].mask_vertex_group = 'M'
bpy.data.objects['EMaskGroup'].modifiers.new(name='Probe', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['EMaskGroup'].modifiers['Probe'].vertex_group = 'G'
bpy.data.objects['EMaskGroup'].modifiers['Probe'].use_add = True
bpy.data.objects['EMaskGroup'].modifiers['Probe'].add_threshold = 0.0
bpy.data.objects['EMaskGroup'].modifiers['Probe'].default_weight = 0.333
bpy.data.objects['EMaskGroup'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['EMaskGroup'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['EMaskGroup'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['EMaskGroup'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['EMaskGroup'].modifiers['Dz'].strength = 1.0
bpy.ops.mesh.primitive_grid_add(x_subdivisions=7, y_subdivisions=7, size=2)
bpy.context.object.name = 'EMaskInv'
bpy.data.objects['EMaskInv'].vertex_groups.new(name='G')
bpy.data.objects['EMaskInv'].vertex_groups['G'].add([3, 17, 33, 36, 38, 49, 59], 0.1, 'REPLACE')
bpy.data.objects['EMaskInv'].vertex_groups['G'].add([23, 37, 54, 55], 0.2, 'REPLACE')
bpy.data.objects['EMaskInv'].vertex_groups['G'].add([6, 11, 14, 18, 61], 0.3, 'REPLACE')
bpy.data.objects['EMaskInv'].vertex_groups['G'].add([2, 13, 22, 51], 0.4, 'REPLACE')
bpy.data.objects['EMaskInv'].vertex_groups['G'].add([0, 7, 16, 40], 0.5, 'REPLACE')
bpy.data.objects['EMaskInv'].vertex_groups['G'].add([8, 46, 47, 60], 0.6, 'REPLACE')
bpy.data.objects['EMaskInv'].vertex_groups['G'].add([26, 52, 57], 0.7, 'REPLACE')
bpy.data.objects['EMaskInv'].vertex_groups['G'].add([1, 9, 24, 25, 30, 43, 48, 56], 0.8, 'REPLACE')
bpy.data.objects['EMaskInv'].vertex_groups['G'].add([19, 27, 58], 0.9, 'REPLACE')
bpy.data.objects['EMaskInv'].vertex_groups['G'].add([42, 63], 1.0, 'REPLACE')
bpy.data.objects['EMaskInv'].vertex_groups.new(name='M')
bpy.data.objects['EMaskInv'].vertex_groups['M'].add([5, 16, 17, 27, 40, 53, 58], 0.1, 'REPLACE')
bpy.data.objects['EMaskInv'].vertex_groups['M'].add([6, 20, 28, 35, 63], 0.2, 'REPLACE')
bpy.data.objects['EMaskInv'].vertex_groups['M'].add([1, 14, 26, 31, 46, 52, 62], 0.3, 'REPLACE')
bpy.data.objects['EMaskInv'].vertex_groups['M'].add([3, 8, 19, 33, 41, 48, 51, 55], 0.4, 'REPLACE')
bpy.data.objects['EMaskInv'].vertex_groups['M'].add([0, 2, 15, 21, 30, 47, 50, 60], 0.5, 'REPLACE')
bpy.data.objects['EMaskInv'].vertex_groups['M'].add([11, 39], 0.6, 'REPLACE')
bpy.data.objects['EMaskInv'].vertex_groups['M'].add([18, 37, 49], 0.7, 'REPLACE')
bpy.data.objects['EMaskInv'].vertex_groups['M'].add([9, 13, 22, 24, 34, 36, 45, 56], 0.8, 'REPLACE')
bpy.data.objects['EMaskInv'].vertex_groups['M'].add([4, 7, 29, 54, 57], 0.9, 'REPLACE')
bpy.data.objects['EMaskInv'].vertex_groups['M'].add([23, 32, 42, 44, 61], 1.0, 'REPLACE')
bpy.data.objects['EMaskInv'].modifiers.new(name='W', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['EMaskInv'].modifiers['W'].vertex_group = 'G'
bpy.data.objects['EMaskInv'].modifiers['W'].falloff_type = 'SHARP'
bpy.data.objects['EMaskInv'].modifiers['W'].mask_vertex_group = 'M'
bpy.data.objects['EMaskInv'].modifiers['W'].invert_mask_vertex_group = True
bpy.data.objects['EMaskInv'].modifiers.new(name='Probe', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['EMaskInv'].modifiers['Probe'].vertex_group = 'G'
bpy.data.objects['EMaskInv'].modifiers['Probe'].use_add = True
bpy.data.objects['EMaskInv'].modifiers['Probe'].add_threshold = 0.0
bpy.data.objects['EMaskInv'].modifiers['Probe'].default_weight = 0.333
bpy.data.objects['EMaskInv'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['EMaskInv'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['EMaskInv'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['EMaskInv'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['EMaskInv'].modifiers['Dz'].strength = 1.0
bpy.ops.mesh.primitive_grid_add(x_subdivisions=7, y_subdivisions=7, size=2)
bpy.context.object.name = 'ENoGroup'
bpy.data.objects['ENoGroup'].vertex_groups.new(name='G')
bpy.data.objects['ENoGroup'].vertex_groups['G'].add([7, 9, 25, 42, 45], 0.1, 'REPLACE')
bpy.data.objects['ENoGroup'].vertex_groups['G'].add([32, 50], 0.2, 'REPLACE')
bpy.data.objects['ENoGroup'].vertex_groups['G'].add([16, 30, 35, 39], 0.3, 'REPLACE')
bpy.data.objects['ENoGroup'].vertex_groups['G'].add([33, 51], 0.4, 'REPLACE')
bpy.data.objects['ENoGroup'].vertex_groups['G'].add([3, 11, 20, 28, 36, 56], 0.5, 'REPLACE')
bpy.data.objects['ENoGroup'].vertex_groups['G'].add([14, 26, 41, 47, 52, 53, 59], 0.6, 'REPLACE')
bpy.data.objects['ENoGroup'].vertex_groups['G'].add([12, 24, 43], 0.7, 'REPLACE')
bpy.data.objects['ENoGroup'].vertex_groups['G'].add([1, 2, 4, 19, 31, 54, 57, 62], 0.8, 'REPLACE')
bpy.data.objects['ENoGroup'].vertex_groups['G'].add([13, 34, 44, 49, 61], 0.9, 'REPLACE')
bpy.data.objects['ENoGroup'].vertex_groups['G'].add([8, 22, 37, 58, 60], 1.0, 'REPLACE')
bpy.data.objects['ENoGroup'].modifiers.new(name='W', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['ENoGroup'].modifiers['W'].vertex_group = 'Nope'
bpy.data.objects['ENoGroup'].modifiers.new(name='Probe', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['ENoGroup'].modifiers['Probe'].vertex_group = 'G'
bpy.data.objects['ENoGroup'].modifiers['Probe'].use_add = True
bpy.data.objects['ENoGroup'].modifiers['Probe'].add_threshold = 0.0
bpy.data.objects['ENoGroup'].modifiers['Probe'].default_weight = 0.333
bpy.data.objects['ENoGroup'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['ENoGroup'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['ENoGroup'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['ENoGroup'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['ENoGroup'].modifiers['Dz'].strength = 1.0
bpy.ops.mesh.primitive_grid_add(x_subdivisions=7, y_subdivisions=7, size=2)
bpy.context.object.name = 'EEmpty'
bpy.data.objects['EEmpty'].vertex_groups.new(name='G')
bpy.data.objects['EEmpty'].modifiers.new(name='W', type='VERTEX_WEIGHT_EDIT')
bpy.data.objects['EEmpty'].modifiers['W'].vertex_group = 'G'
bpy.data.objects['EEmpty'].modifiers['W'].use_add = True
bpy.data.objects['EEmpty'].modifiers['W'].default_weight = 0.7
bpy.data.objects['EEmpty'].modifiers.new(name='Dz', type='DISPLACE')
bpy.data.objects['EEmpty'].modifiers['Dz'].vertex_group = 'G'
bpy.data.objects['EEmpty'].modifiers['Dz'].direction = 'Z'
bpy.data.objects['EEmpty'].modifiers['Dz'].mid_level = 0.0
bpy.data.objects['EEmpty'].modifiers['Dz'].strength = 1.0
