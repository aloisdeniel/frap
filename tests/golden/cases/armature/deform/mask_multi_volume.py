# WP 6.5: ARMATURE modifier options. vertex_group masks the effect (partial weights, inverted,
# a mask group the mesh does not have); use_deform_preserve_volume blends dual quaternions
# (bones with rotation, non-uniform scale and negative-free shear through a scaled parent);
# use_multi_modifier deforms the previous ARMATURE modifier's input and mixes the result with
# its output by the mask group, through a chain of three modifiers with a disabled one in the
# middle; a modifier with no object, and one pointing at a mesh (disabled as in Blender).
# golden: startup=factory tier=T1 tol=1e-5 compare=evaluated:Grid,evaluated:Grid.001,evaluated:Grid.002,evaluated:Grid.003
import bpy
bpy.ops.object.armature_add(location=(0, 0, -1))
bpy.ops.object.mode_set(mode='EDIT')
bpy.data.armatures['Armature'].edit_bones['Bone'].tail = (0, 0, 1.0)
bpy.data.armatures['Armature'].edit_bones.new('Top')
bpy.data.armatures['Armature'].edit_bones['Top'].head = (0, 0, 1.0)
bpy.data.armatures['Armature'].edit_bones['Top'].tail = (0, 0, 2.2)
bpy.data.armatures['Armature'].edit_bones['Top'].parent = bpy.data.armatures['Armature'].edit_bones['Bone']
bpy.data.armatures['Armature'].edit_bones['Top'].use_connect = True
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_grid_add(x_subdivisions=6, y_subdivisions=8, size=2.0, location=(0.1, 0, 0.1), rotation=(1.5707964, 0, 0))
bpy.data.objects['Grid'].vertex_groups.new(name='Bone')
bpy.data.objects['Grid'].vertex_groups.new(name='Top')
bpy.data.objects['Grid'].vertex_groups.new(name='Mask')
bpy.data.objects['Grid'].vertex_groups['Bone'].add([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27], 1.0, 'REPLACE')
bpy.data.objects['Grid'].vertex_groups['Top'].add([21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55], 0.5, 'REPLACE')
bpy.data.objects['Grid'].vertex_groups['Mask'].add([0, 1, 2, 3, 10, 11, 12, 13, 20, 21, 22, 23, 30, 31, 32, 33, 40, 41, 42], 0.25, 'REPLACE')
bpy.data.objects['Grid'].vertex_groups['Mask'].add([4, 14, 24, 34, 44], 1.0, 'REPLACE')
bpy.ops.object.duplicate()
bpy.ops.object.duplicate()
bpy.ops.object.duplicate()
bpy.data.objects['Grid'].modifiers.new('Masked', 'ARMATURE')
bpy.data.objects['Grid'].modifiers['Masked'].object = bpy.data.objects['Armature']
bpy.data.objects['Grid'].modifiers['Masked'].vertex_group = 'Mask'
bpy.data.objects['Grid'].modifiers.new('Inverted', 'ARMATURE')
bpy.data.objects['Grid'].modifiers['Inverted'].object = bpy.data.objects['Armature']
bpy.data.objects['Grid'].modifiers['Inverted'].vertex_group = 'Mask'
bpy.data.objects['Grid'].modifiers['Inverted'].invert_vertex_group = True
bpy.data.objects['Grid'].modifiers.new('Missing', 'ARMATURE')
bpy.data.objects['Grid'].modifiers['Missing'].object = bpy.data.objects['Armature']
bpy.data.objects['Grid'].modifiers['Missing'].vertex_group = 'NoSuchGroup'
bpy.data.objects['Grid.001'].modifiers.new('Volume', 'ARMATURE')
bpy.data.objects['Grid.001'].modifiers['Volume'].object = bpy.data.objects['Armature']
bpy.data.objects['Grid.001'].modifiers['Volume'].use_deform_preserve_volume = True
bpy.data.objects['Grid.001'].modifiers.new('VolumeEnv', 'ARMATURE')
bpy.data.objects['Grid.001'].modifiers['VolumeEnv'].object = bpy.data.objects['Armature']
bpy.data.objects['Grid.001'].modifiers['VolumeEnv'].use_vertex_groups = False
bpy.data.objects['Grid.001'].modifiers['VolumeEnv'].use_bone_envelopes = True
bpy.data.objects['Grid.001'].modifiers['VolumeEnv'].use_deform_preserve_volume = True
bpy.data.objects['Grid.002'].modifiers.new('First', 'ARMATURE')
bpy.data.objects['Grid.002'].modifiers['First'].object = bpy.data.objects['Armature']
bpy.data.objects['Grid.002'].modifiers.new('Second', 'ARMATURE')
bpy.data.objects['Grid.002'].modifiers['Second'].object = bpy.data.objects['Armature']
bpy.data.objects['Grid.002'].modifiers['Second'].use_multi_modifier = True
bpy.data.objects['Grid.002'].modifiers['Second'].vertex_group = 'Mask'
bpy.data.objects['Grid.002'].modifiers['Second'].use_deform_preserve_volume = True
bpy.data.objects['Grid.003'].modifiers.new('A', 'ARMATURE')
bpy.data.objects['Grid.003'].modifiers['A'].object = bpy.data.objects['Armature']
bpy.data.objects['Grid.003'].modifiers.new('B', 'ARMATURE')
bpy.data.objects['Grid.003'].modifiers['B'].object = bpy.data.objects['Armature']
bpy.data.objects['Grid.003'].modifiers['B'].use_multi_modifier = True
bpy.data.objects['Grid.003'].modifiers['B'].show_viewport = False
bpy.data.objects['Grid.003'].modifiers.new('C', 'ARMATURE')
bpy.data.objects['Grid.003'].modifiers['C'].object = bpy.data.objects['Armature']
bpy.data.objects['Grid.003'].modifiers['C'].use_multi_modifier = True
bpy.data.objects['Grid.003'].modifiers['C'].vertex_group = 'Mask'
bpy.data.objects['Grid.003'].modifiers['C'].invert_vertex_group = True
bpy.data.objects['Grid.003'].modifiers.new('None', 'ARMATURE')
bpy.data.objects['Grid.003'].modifiers.new('Mesh', 'ARMATURE')
bpy.data.objects['Grid.003'].modifiers['Mesh'].object = bpy.data.objects['Grid']
bpy.data.objects['Armature'].pose.bones['Bone'].rotation_quaternion = (0.7, 0.1, 0.7, 0.1)
bpy.data.objects['Armature'].pose.bones['Bone'].scale = (1.0, 1.3, 0.7)
bpy.data.objects['Armature'].pose.bones['Top'].rotation_quaternion = (0.6, -0.6, 0.2, 0.3)
bpy.data.objects['Armature'].pose.bones['Top'].scale = (0.8, 1.2, 1.0)
bpy.data.objects['Armature'].pose.bones['Top'].location = (0.1, 0.2, 0.3)
