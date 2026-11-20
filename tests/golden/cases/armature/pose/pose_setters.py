# WP 6.2, PoseBone setters: matrix (through the parent's current pose, no Euler
# compatibility), matrix_basis, rotation_mode (converts the stored rotation), matrix_channel,
# Armature.pose_position='REST' (the rest matrices), renaming a pose bone (renames the bone
# and stays unique), Pose.use_mirror_x.
# golden: startup=factory compare=objects,context tier=T0 tol=1e-5
import bpy
bpy.data.objects.new('C0', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['C0'])
bpy.data.objects.new('C1', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['C1'])
bpy.data.objects.new('C2', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['C2'])
bpy.data.objects.new('C3', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['C3'])
bpy.data.objects.new('C4', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['C4'])
bpy.ops.object.armature_add(location=(0, 1, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.data.armatures['Armature'].edit_bones.new('Child')
bpy.data.armatures['Armature'].edit_bones['Child'].head = (0, 0, 1)
bpy.data.armatures['Armature'].edit_bones['Child'].tail = (0.5, 0.5, 1.5)
bpy.data.armatures['Armature'].edit_bones['Child'].parent = bpy.data.armatures['Armature'].edit_bones['Bone']
bpy.data.armatures['Armature'].edit_bones['Child'].use_connect = True
bpy.ops.object.mode_set(mode='POSE')
bpy.data.objects['Armature'].pose.bones['Bone'].rotation_quaternion = (0.8, 0.1, 0.3, 0.2)
bpy.context.view_layer.update()
bpy.data.objects['Armature'].pose.bones['Child'].rotation_mode = 'XYZ'
bpy.data.objects['Armature'].pose.bones['Child'].matrix = ((0, -1, 0, 0.2), (1, 0, 0, 0.5), (0, 0, 1, 1.4), (0, 0, 0, 1))
bpy.data.objects['C0'].rotation_euler = bpy.data.objects['Armature'].pose.bones['Child'].rotation_euler
bpy.data.objects['C0'].location = bpy.data.objects['Armature'].pose.bones['Child'].location
bpy.data.objects['C0'].scale = bpy.data.objects['Armature'].pose.bones['Child'].scale
bpy.data.objects['Armature'].pose.bones['Bone'].matrix_basis = ((1, 0, 0, 0.3), (0, 0, -2, 0), (0, 1, 0, 0), (0, 0, 0, 1))
bpy.data.objects['C1'].location = bpy.data.objects['Armature'].pose.bones['Bone'].location
bpy.data.objects['C1'].scale = bpy.data.objects['Armature'].pose.bones['Bone'].scale
bpy.data.objects['C1'].rotation_euler = bpy.data.objects['Armature'].pose.bones['Bone'].rotation_quaternion.to_euler()
bpy.data.objects['Armature'].pose.bones['Bone'].rotation_mode = 'AXIS_ANGLE'
bpy.data.objects['C2'].location = bpy.data.objects['Armature'].pose.bones['Bone'].rotation_axis_angle[1:4]
bpy.data.objects['C2'].rotation_euler.x = bpy.data.objects['Armature'].pose.bones['Bone'].rotation_axis_angle[0]
bpy.data.objects['Armature'].pose.bones['Bone'].rotation_mode = 'ZYX'
bpy.data.objects['C2'].scale = bpy.data.objects['Armature'].pose.bones['Bone'].rotation_euler
bpy.context.view_layer.update()
bpy.data.objects['C3'].matrix_world = bpy.data.objects['Armature'].pose.bones['Child'].matrix_channel
bpy.data.objects['C4'].matrix_world = bpy.data.objects['Armature'].pose.bones['Child'].matrix_basis
bpy.data.armatures['Armature'].pose_position = 'REST'
bpy.context.view_layer.update()
bpy.data.objects['C3'].location = bpy.data.objects['Armature'].pose.bones['Child'].tail
bpy.data.armatures['Armature'].pose_position = 'POSE'
bpy.data.objects['Armature'].pose.bones['Child'].name = 'Bone'
bpy.data.armatures['Armature'].bones['Bone.001']
bpy.data.objects['Armature'].pose.bones['Bone.001'].name
bpy.data.objects['Armature'].pose.bones['Bone.001'].name = 'Arm'
bpy.data.armatures['Armature'].bones['Arm'].name = 'Arm.L'
bpy.data.objects['Armature'].pose.bones['Arm.L'].name
bpy.data.objects['Armature'].pose.use_mirror_x = True
bpy.data.objects['C4'].location.x = bpy.data.objects['Armature'].pose.use_mirror_x
bpy.data.objects['Armature'].pose.bones['Arm.L'].rotation_mode = 'BOGUS'
bpy.data.objects['Armature'].pose.bones['Arm.L'].matrix = 3
