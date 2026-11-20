# WP 6.10: WELD restricted to `vertex_group` (only vertices with a weight above 0 merge),
# inverted, in ALL and CONNECTED modes, and with a missing group (the whole mesh merges,
# as without a group).
# golden: startup=factory compare=evaluated:A,evaluated:B,evaluated:C,evaluated:D,evaluated:E,evaluated:F tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=6, y_subdivisions=5, size=2)
bpy.context.object.name = 'A'
bpy.data.objects['A'].vertex_groups.new(name='G')
bpy.data.objects['A'].vertex_groups['G'].add([0, 7, 14, 21, 28, 35], 0.0, 'REPLACE')
bpy.data.objects['A'].vertex_groups['G'].add([1, 8, 15, 22, 29, 36], 0.15, 'REPLACE')
bpy.data.objects['A'].vertex_groups['G'].add([2, 9, 16, 23, 30, 37], 0.35, 'REPLACE')
bpy.data.objects['A'].vertex_groups['G'].add([3, 10, 17, 24, 31, 38], 0.5, 'REPLACE')
bpy.data.objects['A'].vertex_groups['G'].add([4, 11, 18, 25, 32, 39], 0.65, 'REPLACE')
bpy.data.objects['A'].vertex_groups['G'].add([5, 12, 19, 26, 33, 40], 0.85, 'REPLACE')
bpy.data.objects['A'].vertex_groups['G'].add([6, 13, 20, 27, 34, 41], 1.0, 'REPLACE')
bpy.data.objects['A'].modifiers.new(name='M', type='WELD')
bpy.data.objects['A'].modifiers['M'].vertex_group = 'G'
bpy.data.objects['A'].modifiers['M'].merge_threshold = 0.45
bpy.ops.mesh.primitive_grid_add(x_subdivisions=6, y_subdivisions=5, size=2)
bpy.context.object.name = 'B'
bpy.data.objects['B'].vertex_groups.new(name='G')
bpy.data.objects['B'].vertex_groups['G'].add([0, 7, 14, 21, 28, 35], 0.0, 'REPLACE')
bpy.data.objects['B'].vertex_groups['G'].add([1, 8, 15, 22, 29, 36], 0.15, 'REPLACE')
bpy.data.objects['B'].vertex_groups['G'].add([2, 9, 16, 23, 30, 37], 0.35, 'REPLACE')
bpy.data.objects['B'].vertex_groups['G'].add([3, 10, 17, 24, 31, 38], 0.5, 'REPLACE')
bpy.data.objects['B'].vertex_groups['G'].add([4, 11, 18, 25, 32, 39], 0.65, 'REPLACE')
bpy.data.objects['B'].vertex_groups['G'].add([5, 12, 19, 26, 33, 40], 0.85, 'REPLACE')
bpy.data.objects['B'].vertex_groups['G'].add([6, 13, 20, 27, 34, 41], 1.0, 'REPLACE')
bpy.data.objects['B'].modifiers.new(name='M', type='WELD')
bpy.data.objects['B'].modifiers['M'].vertex_group = 'G'
bpy.data.objects['B'].modifiers['M'].merge_threshold = 0.45
bpy.data.objects['B'].modifiers['M'].invert_vertex_group = True
bpy.ops.mesh.primitive_grid_add(x_subdivisions=6, y_subdivisions=5, size=2)
bpy.context.object.name = 'C'
bpy.data.objects['C'].vertex_groups.new(name='G')
bpy.data.objects['C'].vertex_groups['G'].add([5, 10, 20, 25, 35, 40], 0.25, 'REPLACE')
bpy.data.objects['C'].vertex_groups['G'].add([8, 13, 23, 28, 38], 0.4, 'REPLACE')
bpy.data.objects['C'].vertex_groups['G'].add([1, 11, 16, 26, 31, 41], 0.5, 'REPLACE')
bpy.data.objects['C'].vertex_groups['G'].add([4, 14, 19, 29, 34], 0.6, 'REPLACE')
bpy.data.objects['C'].vertex_groups['G'].add([2, 7, 17, 22, 32, 37], 0.75, 'REPLACE')
bpy.data.objects['C'].modifiers.new(name='M', type='WELD')
bpy.data.objects['C'].modifiers['M'].vertex_group = 'G'
bpy.data.objects['C'].modifiers['M'].merge_threshold = 0.45
bpy.data.objects['C'].modifiers['M'].mode = 'CONNECTED'
bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=2, radius=1)
bpy.context.object.name = 'D'
bpy.data.objects['D'].vertex_groups.new(name='G')
bpy.data.objects['D'].vertex_groups['G'].add([1, 5, 6, 9, 10, 13, 14, 15, 16, 21, 22, 23, 25, 30, 32, 36, 37, 38, 41], 1.0, 'REPLACE')
bpy.data.objects['D'].modifiers.new(name='M', type='WELD')
bpy.data.objects['D'].modifiers['M'].vertex_group = 'G'
bpy.data.objects['D'].modifiers['M'].merge_threshold = 0.6
bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=2, radius=1)
bpy.context.object.name = 'E'
bpy.data.objects['E'].vertex_groups.new(name='G')
bpy.data.objects['E'].vertex_groups['G'].add([1, 5, 6, 9, 10, 13, 14, 15, 16, 21, 22, 23, 25, 30, 32, 36, 37, 38, 41], 1.0, 'REPLACE')
bpy.data.objects['E'].modifiers.new(name='M', type='WELD')
bpy.data.objects['E'].modifiers['M'].vertex_group = 'G'
bpy.data.objects['E'].modifiers['M'].merge_threshold = 0.6
bpy.data.objects['E'].modifiers['M'].mode = 'CONNECTED'
bpy.data.objects['E'].modifiers['M'].invert_vertex_group = True
bpy.ops.mesh.primitive_grid_add(x_subdivisions=6, y_subdivisions=5, size=2)
bpy.context.object.name = 'F'
bpy.data.objects['F'].vertex_groups.new(name='H')
bpy.data.objects['F'].vertex_groups['H'].add([4, 5, 6, 11, 12, 13, 18, 19, 20, 25, 26, 27, 32, 33, 34, 39, 40, 41], 1.0, 'REPLACE')
bpy.data.objects['F'].modifiers.new(name='M', type='WELD')
bpy.data.objects['F'].modifiers['M'].vertex_group = 'G'
bpy.data.objects['F'].modifiers['M'].merge_threshold = 0.45
