# WP 6.2, pose.select_all, pose.loc_clear / rot_clear / scale_clear / transforms_clear (on the
# selected visible bones, honouring locks, Euler-compatible locked quaternions), with
# Pose.use_mirror_x, pose.rotation_mode_set (converts each selected bone), and
# pose.armature_apply (all bones, and selected=True keeping unselected children in place).
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
bpy.data.objects.new('C5', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['C5'])
bpy.data.objects.new('C6', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['C6'])
bpy.data.objects.new('C7', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['C7'])
bpy.ops.object.armature_add()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.armature.extrude_move(TRANSFORM_OT_translate={'value': (0.5, 0, 1)})
bpy.ops.armature.extrude_move(TRANSFORM_OT_translate={'value': (0, 0.5, 1)})
bpy.data.armatures['Armature'].edit_bones.new('a.L')
bpy.data.armatures['Armature'].edit_bones['a.L'].head = (1, 0, 0)
bpy.data.armatures['Armature'].edit_bones['a.L'].tail = (2, 0, 0)
bpy.data.armatures['Armature'].edit_bones.new('a.R')
bpy.data.armatures['Armature'].edit_bones['a.R'].head = (-1, 0, 0)
bpy.data.armatures['Armature'].edit_bones['a.R'].tail = (-2, 0, 0)
bpy.ops.object.mode_set(mode='POSE')
bpy.ops.pose.loc_clear()
bpy.ops.pose.select_all(action='SELECT')
bpy.data.objects['Armature'].pose.bones['Bone'].rotation_quaternion = (0.9, 0.2, 0.3, 0.1)
bpy.data.objects['Armature'].pose.bones['Bone'].location = (0.1, 0.2, 0.3)
bpy.data.objects['Armature'].pose.bones['Bone'].scale = (2, 3, 4)
bpy.data.objects['Armature'].pose.bones['Bone.001'].rotation_mode = 'YZX'
bpy.data.objects['Armature'].pose.bones['Bone.001'].rotation_euler = (0.4, 0.5, 0.6)
bpy.data.objects['Armature'].pose.bones['Bone.001'].location = (1, 1, 1)
bpy.data.objects['Armature'].pose.bones['Bone.002'].rotation_quaternion = (0.5, 0.5, 0.5, 0.5)
bpy.data.objects['Armature'].pose.bones['Bone.002'].scale = (0.5, 0.5, 0.5)
bpy.data.objects['Armature'].pose.bones['a.L'].location = (0.3, 0.3, 0.3)
bpy.data.objects['Armature'].pose.bones['a.R'].location = (0.6, 0.6, 0.6)
bpy.data.objects['Armature'].pose.bones['a.R'].select = False
bpy.context.view_layer.update()
bpy.ops.pose.armature_apply(selected=True)
bpy.data.objects['C0'].matrix_world = bpy.data.armatures['Armature'].bones['Bone.001'].matrix_local
bpy.data.objects['C1'].matrix_world = bpy.data.armatures['Armature'].bones['Bone.002'].matrix_local
bpy.data.objects['C2'].location = bpy.data.objects['Armature'].pose.bones['Bone.001'].location
bpy.data.objects['C2'].rotation_euler = bpy.data.objects['Armature'].pose.bones['a.R'].location
bpy.data.objects['C2'].scale = bpy.data.armatures['Armature'].bones['Bone.002'].tail_local
bpy.data.objects['Armature'].pose.bones['Bone'].rotation_quaternion = (0.8, 0.1, 0.4, 0.2)
bpy.data.objects['Armature'].pose.bones['Bone'].lock_rotation = (True, False, False)
bpy.data.objects['Armature'].pose.bones['Bone'].lock_rotations_4d = False
bpy.data.objects['Armature'].pose.bones['Bone'].location = (0.1, 0.2, 0.3)
bpy.data.objects['Armature'].pose.bones['Bone'].lock_location = (False, True, False)
bpy.data.objects['Armature'].pose.bones['Bone'].scale = (2, 3, 4)
bpy.data.objects['Armature'].pose.bones['Bone'].lock_scale = (False, False, True)
bpy.data.objects['Armature'].pose.bones['Bone.001'].rotation_euler = (0.4, 0.5, 0.6)
bpy.data.objects['Armature'].pose.bones['Bone.001'].lock_rotation = (False, True, False)
bpy.data.objects['Armature'].pose.bones['Bone.002'].rotation_mode = 'AXIS_ANGLE'
bpy.data.objects['Armature'].pose.bones['Bone.002'].rotation_axis_angle = (0.8, 0.2, 0.6, 0.3)
bpy.data.objects['Armature'].pose.bones['Bone.002'].lock_rotation_w = True
bpy.ops.pose.transforms_clear()
bpy.data.objects['C3'].location = bpy.data.objects['Armature'].pose.bones['Bone'].location
bpy.data.objects['C3'].rotation_euler = bpy.data.objects['Armature'].pose.bones['Bone'].rotation_quaternion[1:4]
bpy.data.objects['C3'].scale = bpy.data.objects['Armature'].pose.bones['Bone'].scale
bpy.data.objects['C4'].location.x = bpy.data.objects['Armature'].pose.bones['Bone'].rotation_quaternion[0]
bpy.data.objects['C4'].rotation_euler = bpy.data.objects['Armature'].pose.bones['Bone.001'].rotation_euler
bpy.data.objects['C4'].scale = bpy.data.objects['Armature'].pose.bones['Bone.002'].rotation_axis_angle[1:4]
bpy.data.objects['C4'].location.y = bpy.data.objects['Armature'].pose.bones['Bone.002'].rotation_axis_angle[0]
bpy.data.objects['Armature'].pose.use_mirror_x = True
bpy.data.objects['Armature'].pose.bones['a.R'].location = (0.6, 0.6, 0.6)
bpy.data.objects['Armature'].pose.bones['a.L'].location = (0.3, 0.3, 0.3)
bpy.ops.pose.select_all(action='DESELECT')
bpy.data.objects['Armature'].pose.bones['a.L'].select = True
bpy.ops.pose.loc_clear()
bpy.data.objects['C5'].location = bpy.data.objects['Armature'].pose.bones['a.R'].location
bpy.data.objects['Armature'].pose.bones['a.L'].scale = (2, 2, 2)
bpy.ops.pose.scale_clear()
bpy.data.objects['Armature'].pose.bones['a.L'].rotation_quaternion = (0, 1, 0, 0)
bpy.ops.pose.rot_clear()
bpy.data.objects['C5'].rotation_euler = bpy.data.objects['Armature'].pose.bones['a.L'].rotation_quaternion[1:4]
bpy.data.objects['Armature'].pose.use_mirror_x = False
bpy.ops.pose.select_all(action='SELECT')
bpy.data.objects['Armature'].pose.bones['Bone.002'].rotation_axis_angle = (0.8, 0.2, 0.6, 0.3)
bpy.data.objects['Armature'].pose.bones['Bone'].rotation_quaternion = (0.8, 0.1, 0.4, 0.2)
bpy.ops.pose.rotation_mode_set(type='XZY')
bpy.data.objects['C5'].scale = bpy.data.objects['Armature'].pose.bones['Bone.002'].rotation_euler
bpy.data.objects['C6'].location = bpy.data.objects['Armature'].pose.bones['Bone'].rotation_euler
bpy.data.objects['Armature'].pose.bones['Bone'].rotation_euler = (0.3, 0, 0)
bpy.data.objects['Armature'].pose.bones['Bone.001'].location = (0.5, 0, 0)
bpy.data.objects['Armature'].pose.bones['Bone.002'].scale = (1, 2, 1)
bpy.context.view_layer.update()
bpy.ops.pose.armature_apply()
bpy.data.objects['C6'].matrix_world = bpy.data.armatures['Armature'].bones['Bone.002'].matrix_local
bpy.data.objects['C7'].location = bpy.data.armatures['Armature'].bones['Bone.001'].head_local
bpy.data.objects['C7'].rotation_euler = bpy.data.objects['Armature'].pose.bones['Bone'].rotation_euler
bpy.data.objects['C7'].scale = bpy.data.objects['Armature'].pose.bones['Bone.002'].scale
bpy.ops.pose.select_all(action='DESELECT')
bpy.ops.pose.armature_apply(selected=True)
bpy.ops.pose.transforms_clear()
bpy.ops.pose.select_all(action='INVERT')
bpy.context.selected_pose_bones[4].name
