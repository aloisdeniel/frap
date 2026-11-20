# WP 6.10: MASK in ARMATURE mode: a vertex is kept when a selected pose bone of the armature has
# a vertex group (named like it) weighing it above the threshold; inverted; the selection read when
# the mesh is evaluated (bones selected after the modifier was set); `pose.select_all`; an
# armature without a pose built (`objects.new`, nothing selected: empty); no armature
# (unchanged).
# golden: startup=factory compare=evaluated:A,evaluated:B,evaluated:C,evaluated:D,evaluated:E tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=6, y_subdivisions=5, size=2)
bpy.context.object.name = 'A'
bpy.data.objects['A'].vertex_groups.new(name='Bone')
bpy.data.objects['A'].vertex_groups['Bone'].add([0, 7, 14, 21, 28, 35], 0.0, 'REPLACE')
bpy.data.objects['A'].vertex_groups['Bone'].add([1, 8, 15, 22, 29, 36], 0.15, 'REPLACE')
bpy.data.objects['A'].vertex_groups['Bone'].add([2, 9, 16, 23, 30, 37], 0.35, 'REPLACE')
bpy.data.objects['A'].vertex_groups['Bone'].add([3, 10, 17, 24, 31, 38], 0.5, 'REPLACE')
bpy.data.objects['A'].vertex_groups['Bone'].add([4, 11, 18, 25, 32, 39], 0.65, 'REPLACE')
bpy.data.objects['A'].vertex_groups['Bone'].add([5, 12, 19, 26, 33, 40], 0.85, 'REPLACE')
bpy.data.objects['A'].vertex_groups['Bone'].add([6, 13, 20, 27, 34, 41], 1.0, 'REPLACE')
bpy.data.objects['A'].vertex_groups.new(name='B2')
bpy.data.objects['A'].vertex_groups['B2'].add([21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41], 1.0, 'REPLACE')
bpy.data.objects['A'].vertex_groups.new(name='Other')
bpy.data.objects['A'].vertex_groups['Other'].add([5, 10, 20, 25, 35, 40], 0.25, 'REPLACE')
bpy.data.objects['A'].vertex_groups['Other'].add([8, 13, 23, 28, 38], 0.4, 'REPLACE')
bpy.data.objects['A'].vertex_groups['Other'].add([1, 11, 16, 26, 31, 41], 0.5, 'REPLACE')
bpy.data.objects['A'].vertex_groups['Other'].add([4, 14, 19, 29, 34], 0.6, 'REPLACE')
bpy.data.objects['A'].vertex_groups['Other'].add([2, 7, 17, 22, 32, 37], 0.75, 'REPLACE')
bpy.data.objects['A'].modifiers.new(name='M', type='MASK')
bpy.data.objects['A'].modifiers['M'].mode = 'ARMATURE'
bpy.data.objects['A'].modifiers['M'].threshold = 0.3
bpy.ops.mesh.primitive_grid_add(x_subdivisions=6, y_subdivisions=5, size=2)
bpy.context.object.name = 'B'
bpy.data.objects['B'].vertex_groups.new(name='Bone')
bpy.data.objects['B'].vertex_groups['Bone'].add([0, 7, 14, 21, 28, 35], 0.0, 'REPLACE')
bpy.data.objects['B'].vertex_groups['Bone'].add([1, 8, 15, 22, 29, 36], 0.15, 'REPLACE')
bpy.data.objects['B'].vertex_groups['Bone'].add([2, 9, 16, 23, 30, 37], 0.35, 'REPLACE')
bpy.data.objects['B'].vertex_groups['Bone'].add([3, 10, 17, 24, 31, 38], 0.5, 'REPLACE')
bpy.data.objects['B'].vertex_groups['Bone'].add([4, 11, 18, 25, 32, 39], 0.65, 'REPLACE')
bpy.data.objects['B'].vertex_groups['Bone'].add([5, 12, 19, 26, 33, 40], 0.85, 'REPLACE')
bpy.data.objects['B'].vertex_groups['Bone'].add([6, 13, 20, 27, 34, 41], 1.0, 'REPLACE')
bpy.data.objects['B'].vertex_groups.new(name='B2')
bpy.data.objects['B'].vertex_groups['B2'].add([21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41], 1.0, 'REPLACE')
bpy.data.objects['B'].vertex_groups.new(name='Other')
bpy.data.objects['B'].vertex_groups['Other'].add([5, 10, 20, 25, 35, 40], 0.25, 'REPLACE')
bpy.data.objects['B'].vertex_groups['Other'].add([8, 13, 23, 28, 38], 0.4, 'REPLACE')
bpy.data.objects['B'].vertex_groups['Other'].add([1, 11, 16, 26, 31, 41], 0.5, 'REPLACE')
bpy.data.objects['B'].vertex_groups['Other'].add([4, 14, 19, 29, 34], 0.6, 'REPLACE')
bpy.data.objects['B'].vertex_groups['Other'].add([2, 7, 17, 22, 32, 37], 0.75, 'REPLACE')
bpy.data.objects['B'].modifiers.new(name='M', type='MASK')
bpy.data.objects['B'].modifiers['M'].mode = 'ARMATURE'
bpy.data.objects['B'].modifiers['M'].threshold = 0.3
bpy.ops.mesh.primitive_grid_add(x_subdivisions=6, y_subdivisions=5, size=2)
bpy.context.object.name = 'C'
bpy.data.objects['C'].vertex_groups.new(name='Bone')
bpy.data.objects['C'].vertex_groups['Bone'].add([0, 7, 14, 21, 28, 35], 0.0, 'REPLACE')
bpy.data.objects['C'].vertex_groups['Bone'].add([1, 8, 15, 22, 29, 36], 0.15, 'REPLACE')
bpy.data.objects['C'].vertex_groups['Bone'].add([2, 9, 16, 23, 30, 37], 0.35, 'REPLACE')
bpy.data.objects['C'].vertex_groups['Bone'].add([3, 10, 17, 24, 31, 38], 0.5, 'REPLACE')
bpy.data.objects['C'].vertex_groups['Bone'].add([4, 11, 18, 25, 32, 39], 0.65, 'REPLACE')
bpy.data.objects['C'].vertex_groups['Bone'].add([5, 12, 19, 26, 33, 40], 0.85, 'REPLACE')
bpy.data.objects['C'].vertex_groups['Bone'].add([6, 13, 20, 27, 34, 41], 1.0, 'REPLACE')
bpy.data.objects['C'].vertex_groups.new(name='B2')
bpy.data.objects['C'].vertex_groups['B2'].add([21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41], 1.0, 'REPLACE')
bpy.data.objects['C'].vertex_groups.new(name='Other')
bpy.data.objects['C'].vertex_groups['Other'].add([5, 10, 20, 25, 35, 40], 0.25, 'REPLACE')
bpy.data.objects['C'].vertex_groups['Other'].add([8, 13, 23, 28, 38], 0.4, 'REPLACE')
bpy.data.objects['C'].vertex_groups['Other'].add([1, 11, 16, 26, 31, 41], 0.5, 'REPLACE')
bpy.data.objects['C'].vertex_groups['Other'].add([4, 14, 19, 29, 34], 0.6, 'REPLACE')
bpy.data.objects['C'].vertex_groups['Other'].add([2, 7, 17, 22, 32, 37], 0.75, 'REPLACE')
bpy.data.objects['C'].modifiers.new(name='M', type='MASK')
bpy.data.objects['C'].modifiers['M'].mode = 'ARMATURE'
bpy.data.objects['C'].modifiers['M'].threshold = 0.3
bpy.ops.mesh.primitive_grid_add(x_subdivisions=6, y_subdivisions=5, size=2)
bpy.context.object.name = 'D'
bpy.data.objects['D'].vertex_groups.new(name='Bone')
bpy.data.objects['D'].vertex_groups['Bone'].add([0, 7, 14, 21, 28, 35], 0.0, 'REPLACE')
bpy.data.objects['D'].vertex_groups['Bone'].add([1, 8, 15, 22, 29, 36], 0.15, 'REPLACE')
bpy.data.objects['D'].vertex_groups['Bone'].add([2, 9, 16, 23, 30, 37], 0.35, 'REPLACE')
bpy.data.objects['D'].vertex_groups['Bone'].add([3, 10, 17, 24, 31, 38], 0.5, 'REPLACE')
bpy.data.objects['D'].vertex_groups['Bone'].add([4, 11, 18, 25, 32, 39], 0.65, 'REPLACE')
bpy.data.objects['D'].vertex_groups['Bone'].add([5, 12, 19, 26, 33, 40], 0.85, 'REPLACE')
bpy.data.objects['D'].vertex_groups['Bone'].add([6, 13, 20, 27, 34, 41], 1.0, 'REPLACE')
bpy.data.objects['D'].vertex_groups.new(name='B2')
bpy.data.objects['D'].vertex_groups['B2'].add([21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41], 1.0, 'REPLACE')
bpy.data.objects['D'].vertex_groups.new(name='Other')
bpy.data.objects['D'].vertex_groups['Other'].add([5, 10, 20, 25, 35, 40], 0.25, 'REPLACE')
bpy.data.objects['D'].vertex_groups['Other'].add([8, 13, 23, 28, 38], 0.4, 'REPLACE')
bpy.data.objects['D'].vertex_groups['Other'].add([1, 11, 16, 26, 31, 41], 0.5, 'REPLACE')
bpy.data.objects['D'].vertex_groups['Other'].add([4, 14, 19, 29, 34], 0.6, 'REPLACE')
bpy.data.objects['D'].vertex_groups['Other'].add([2, 7, 17, 22, 32, 37], 0.75, 'REPLACE')
bpy.data.objects['D'].modifiers.new(name='M', type='MASK')
bpy.data.objects['D'].modifiers['M'].mode = 'ARMATURE'
bpy.data.objects['D'].modifiers['M'].threshold = 0.3
bpy.ops.mesh.primitive_grid_add(x_subdivisions=6, y_subdivisions=5, size=2)
bpy.context.object.name = 'E'
bpy.data.objects['E'].vertex_groups.new(name='Bone')
bpy.data.objects['E'].vertex_groups['Bone'].add([0, 7, 14, 21, 28, 35], 0.0, 'REPLACE')
bpy.data.objects['E'].vertex_groups['Bone'].add([1, 8, 15, 22, 29, 36], 0.15, 'REPLACE')
bpy.data.objects['E'].vertex_groups['Bone'].add([2, 9, 16, 23, 30, 37], 0.35, 'REPLACE')
bpy.data.objects['E'].vertex_groups['Bone'].add([3, 10, 17, 24, 31, 38], 0.5, 'REPLACE')
bpy.data.objects['E'].vertex_groups['Bone'].add([4, 11, 18, 25, 32, 39], 0.65, 'REPLACE')
bpy.data.objects['E'].vertex_groups['Bone'].add([5, 12, 19, 26, 33, 40], 0.85, 'REPLACE')
bpy.data.objects['E'].vertex_groups['Bone'].add([6, 13, 20, 27, 34, 41], 1.0, 'REPLACE')
bpy.data.objects['E'].vertex_groups.new(name='B2')
bpy.data.objects['E'].vertex_groups['B2'].add([21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41], 1.0, 'REPLACE')
bpy.data.objects['E'].vertex_groups.new(name='Other')
bpy.data.objects['E'].vertex_groups['Other'].add([5, 10, 20, 25, 35, 40], 0.25, 'REPLACE')
bpy.data.objects['E'].vertex_groups['Other'].add([8, 13, 23, 28, 38], 0.4, 'REPLACE')
bpy.data.objects['E'].vertex_groups['Other'].add([1, 11, 16, 26, 31, 41], 0.5, 'REPLACE')
bpy.data.objects['E'].vertex_groups['Other'].add([4, 14, 19, 29, 34], 0.6, 'REPLACE')
bpy.data.objects['E'].vertex_groups['Other'].add([2, 7, 17, 22, 32, 37], 0.75, 'REPLACE')
bpy.data.objects['E'].modifiers.new(name='M', type='MASK')
bpy.data.objects['E'].modifiers['M'].mode = 'ARMATURE'
bpy.data.objects['E'].modifiers['M'].threshold = 0.3
bpy.data.objects['B'].modifiers['M'].invert_vertex_group = True
bpy.ops.object.armature_add()
bpy.ops.object.mode_set(mode='EDIT')
bpy.data.armatures['Armature'].edit_bones.new('B2')
bpy.data.armatures['Armature'].edit_bones['B2'].tail = (0, 1, 1)
bpy.data.armatures['Armature'].edit_bones.new('Other')
bpy.data.armatures['Armature'].edit_bones['Other'].tail = (1, 0, 1)
bpy.ops.object.mode_set(mode='POSE')
bpy.ops.pose.select_all(action='DESELECT')
bpy.data.objects['Armature'].pose.bones['B2'].select = True
bpy.data.objects['A'].modifiers['M'].armature = bpy.data.objects['Armature']
bpy.data.objects['B'].modifiers['M'].armature = bpy.data.objects['Armature']
bpy.data.objects['Armature'].pose.bones['Bone'].select = True
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.object.armature_add(location=(3, 0, 0))
bpy.ops.object.mode_set(mode='POSE')
bpy.ops.pose.select_all(action='SELECT')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['C'].modifiers['M'].armature = bpy.data.objects['Armature.001']
bpy.data.objects.new('Bare', bpy.data.armatures.new('Bare'))
bpy.data.objects['D'].modifiers['M'].armature = bpy.data.objects['Bare']
