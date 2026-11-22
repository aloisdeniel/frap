# WP 8.3.11, pose.quaternions_flip (selected visible quaternion bones, CANCELLED without one),
# pose.visual_transform_apply (parents and children together, a non-uniformly scaled parent
# shearing its children, Euler-compatible results in every rotation mode, inherit_scale NONE,
# the rest position), pose.user_transforms_clear (no action: the rest pose of the selected or
# every bone; with an action: the keyed channels back to their values at the current frame,
# the others untouched) and the Pose Mode polls. P<n>/Q<n> hold location, Euler, scale,
# quaternion and axis-angle of one bone.
# golden: startup=factory compare=objects tier=T1 tol=1e-5
import bpy
bpy.ops.object.armature_add()
bpy.ops.object.mode_set(mode='EDIT')
bpy.data.armatures['Armature'].edit_bones.new('arm.L')
bpy.data.armatures['Armature'].edit_bones['arm.L'].head = (1, 0, 0.5)
bpy.data.armatures['Armature'].edit_bones['arm.L'].tail = (2, 0, 0.5)
bpy.data.armatures['Armature'].edit_bones['arm.L'].parent = bpy.data.armatures['Armature'].edit_bones['Bone']
bpy.data.armatures['Armature'].edit_bones.new('arm.R')
bpy.data.armatures['Armature'].edit_bones['arm.R'].head = (-1, 0, 0.5)
bpy.data.armatures['Armature'].edit_bones['arm.R'].tail = (-2, 0, 0.5)
bpy.data.armatures['Armature'].edit_bones['arm.R'].parent = bpy.data.armatures['Armature'].edit_bones['Bone']
bpy.data.armatures['Armature'].edit_bones.new('hand.L')
bpy.data.armatures['Armature'].edit_bones['hand.L'].head = (2, 0, 0.5)
bpy.data.armatures['Armature'].edit_bones['hand.L'].tail = (3, 0, 0.7)
bpy.data.armatures['Armature'].edit_bones['hand.L'].parent = bpy.data.armatures['Armature'].edit_bones['arm.L']
bpy.data.armatures['Armature'].edit_bones['hand.L'].use_connect = True
bpy.data.armatures['Armature'].edit_bones.new('hand.R')
bpy.data.armatures['Armature'].edit_bones['hand.R'].head = (-2, 0, 0.5)
bpy.data.armatures['Armature'].edit_bones['hand.R'].tail = (-3, 0, 0.7)
bpy.data.armatures['Armature'].edit_bones['hand.R'].parent = bpy.data.armatures['Armature'].edit_bones['arm.R']
bpy.data.armatures['Armature'].edit_bones['hand.R'].use_connect = True
bpy.data.armatures['Armature'].edit_bones.new('B1')
bpy.data.armatures['Armature'].edit_bones['B1'].head = (0, 0, 1)
bpy.data.armatures['Armature'].edit_bones['B1'].tail = (0, 0.3, 2)
bpy.data.armatures['Armature'].edit_bones['B1'].parent = bpy.data.armatures['Armature'].edit_bones['Bone']
bpy.data.armatures['Armature'].edit_bones['B1'].use_connect = True
bpy.data.armatures['Armature'].edit_bones.new('leg.L')
bpy.data.armatures['Armature'].edit_bones['leg.L'].head = (0.4, 0, 0)
bpy.data.armatures['Armature'].edit_bones['leg.L'].tail = (0.5, 0.1, -1)
bpy.data.armatures['Armature'].edit_bones.new('leg.R')
bpy.data.armatures['Armature'].edit_bones['leg.R'].head = (-0.4, 0, 0)
bpy.data.armatures['Armature'].edit_bones['leg.R'].tail = (-0.5, 0.1, -1)
bpy.ops.object.mode_set(mode='POSE')
bpy.data.objects['Armature'].pose.bones['arm.L'].rotation_quaternion = (0.9, 0.1, 0.3, 0.2)
bpy.data.objects['Armature'].pose.bones['arm.R'].rotation_mode = 'XYZ'
bpy.data.objects['Armature'].pose.bones['arm.R'].rotation_euler = (0.3, 0.2, 0.1)
bpy.data.objects['Armature'].pose.bones['hand.L'].rotation_quaternion = (-0.5, 0.5, 0.5, 0.5)
bpy.data.objects['Armature'].pose.bones['hand.R'].rotation_quaternion = (0.6, 0, 0.8, 0)
bpy.ops.pose.select_all(action='DESELECT')
bpy.data.objects['Armature'].pose.bones['arm.L'].select = True
bpy.data.objects['Armature'].pose.bones['arm.R'].select = True
bpy.data.objects['Armature'].pose.bones['hand.L'].select = True
bpy.data.objects['Armature'].pose.bones['hand.R'].select = True
bpy.data.objects['Armature'].pose.bones['hand.R'].hide = True
bpy.ops.pose.quaternions_flip()
bpy.data.objects.new('P0', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P0'])
bpy.data.objects['P0'].location = bpy.data.objects['Armature'].pose.bones['arm.L'].location
bpy.data.objects['P0'].rotation_euler = bpy.data.objects['Armature'].pose.bones['arm.L'].rotation_euler
bpy.data.objects['P0'].scale = bpy.data.objects['Armature'].pose.bones['arm.L'].scale
bpy.data.objects.new('Q0', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Q0'])
bpy.data.objects['Q0'].location = bpy.data.objects['Armature'].pose.bones['arm.L'].rotation_quaternion[1:4]
bpy.data.objects['Q0'].scale.x = bpy.data.objects['Armature'].pose.bones['arm.L'].rotation_quaternion[0]
bpy.data.objects['Q0'].rotation_euler = bpy.data.objects['Armature'].pose.bones['arm.L'].rotation_axis_angle[1:4]
bpy.data.objects['Q0'].scale.y = bpy.data.objects['Armature'].pose.bones['arm.L'].rotation_axis_angle[0]
bpy.data.objects.new('P1', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P1'])
bpy.data.objects['P1'].location = bpy.data.objects['Armature'].pose.bones['arm.R'].location
bpy.data.objects['P1'].rotation_euler = bpy.data.objects['Armature'].pose.bones['arm.R'].rotation_euler
bpy.data.objects['P1'].scale = bpy.data.objects['Armature'].pose.bones['arm.R'].scale
bpy.data.objects.new('Q1', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Q1'])
bpy.data.objects['Q1'].location = bpy.data.objects['Armature'].pose.bones['arm.R'].rotation_quaternion[1:4]
bpy.data.objects['Q1'].scale.x = bpy.data.objects['Armature'].pose.bones['arm.R'].rotation_quaternion[0]
bpy.data.objects['Q1'].rotation_euler = bpy.data.objects['Armature'].pose.bones['arm.R'].rotation_axis_angle[1:4]
bpy.data.objects['Q1'].scale.y = bpy.data.objects['Armature'].pose.bones['arm.R'].rotation_axis_angle[0]
bpy.data.objects.new('P2', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P2'])
bpy.data.objects['P2'].location = bpy.data.objects['Armature'].pose.bones['hand.L'].location
bpy.data.objects['P2'].rotation_euler = bpy.data.objects['Armature'].pose.bones['hand.L'].rotation_euler
bpy.data.objects['P2'].scale = bpy.data.objects['Armature'].pose.bones['hand.L'].scale
bpy.data.objects.new('Q2', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Q2'])
bpy.data.objects['Q2'].location = bpy.data.objects['Armature'].pose.bones['hand.L'].rotation_quaternion[1:4]
bpy.data.objects['Q2'].scale.x = bpy.data.objects['Armature'].pose.bones['hand.L'].rotation_quaternion[0]
bpy.data.objects['Q2'].rotation_euler = bpy.data.objects['Armature'].pose.bones['hand.L'].rotation_axis_angle[1:4]
bpy.data.objects['Q2'].scale.y = bpy.data.objects['Armature'].pose.bones['hand.L'].rotation_axis_angle[0]
bpy.data.objects.new('P3', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P3'])
bpy.data.objects['P3'].location = bpy.data.objects['Armature'].pose.bones['hand.R'].location
bpy.data.objects['P3'].rotation_euler = bpy.data.objects['Armature'].pose.bones['hand.R'].rotation_euler
bpy.data.objects['P3'].scale = bpy.data.objects['Armature'].pose.bones['hand.R'].scale
bpy.data.objects.new('Q3', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Q3'])
bpy.data.objects['Q3'].location = bpy.data.objects['Armature'].pose.bones['hand.R'].rotation_quaternion[1:4]
bpy.data.objects['Q3'].scale.x = bpy.data.objects['Armature'].pose.bones['hand.R'].rotation_quaternion[0]
bpy.data.objects['Q3'].rotation_euler = bpy.data.objects['Armature'].pose.bones['hand.R'].rotation_axis_angle[1:4]
bpy.data.objects['Q3'].scale.y = bpy.data.objects['Armature'].pose.bones['hand.R'].rotation_axis_angle[0]
bpy.data.objects['Armature'].pose.bones['hand.R'].hide = False
bpy.ops.pose.select_all(action='DESELECT')
bpy.data.objects['Armature'].pose.bones['arm.R'].select = True
bpy.ops.pose.quaternions_flip()
bpy.data.objects['Armature'].pose.bones['Bone'].location = (0.2, 0.1, 0)
bpy.data.objects['Armature'].pose.bones['Bone'].rotation_quaternion = (0.8, 0.2, 0.5, 0.1)
bpy.data.objects['Armature'].pose.bones['Bone'].scale = (1, 2.5, 0.5)
bpy.data.objects['Armature'].pose.bones['arm.L'].location = (0.3, 0, 0.2)
bpy.data.objects['Armature'].pose.bones['arm.L'].rotation_quaternion = (0.6, 0.6, 0.2, 0.4)
bpy.data.objects['Armature'].pose.bones['arm.L'].scale = (1.5, 1, 1)
bpy.data.objects['Armature'].pose.bones['hand.L'].rotation_mode = 'XZY'
bpy.data.objects['Armature'].pose.bones['hand.L'].rotation_euler = (0.5, -0.4, 2.5)
bpy.data.objects['Armature'].pose.bones['hand.L'].scale = (1, 1, 3)
bpy.data.objects['Armature'].pose.bones['B1'].rotation_mode = 'AXIS_ANGLE'
bpy.data.objects['Armature'].pose.bones['B1'].rotation_axis_angle = (2.0, 0.3, 0.9, 0.1)
bpy.data.objects['Armature'].pose.bones['leg.L'].rotation_mode = 'YXZ'
bpy.data.objects['Armature'].pose.bones['leg.L'].rotation_euler = (3.0, 0.2, -2.9)
bpy.data.armatures['Armature'].bones['hand.R'].inherit_scale = 'NONE'
bpy.data.objects['Armature'].pose.bones['hand.R'].location = (0.1, 0.2, 0.3)
bpy.data.objects['Armature'].pose.bones['hand.R'].rotation_quaternion = (0.3, 0.1, 0.9, 0.2)
bpy.ops.pose.select_all(action='DESELECT')
bpy.data.objects['Armature'].pose.bones['Bone'].select = True
bpy.data.objects['Armature'].pose.bones['arm.L'].select = True
bpy.data.objects['Armature'].pose.bones['hand.L'].select = True
bpy.data.objects['Armature'].pose.bones['B1'].select = True
bpy.data.objects['Armature'].pose.bones['leg.L'].select = True
bpy.data.objects['Armature'].pose.bones['hand.R'].select = True
bpy.ops.pose.visual_transform_apply()
bpy.data.objects.new('P4', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P4'])
bpy.data.objects['P4'].location = bpy.data.objects['Armature'].pose.bones['Bone'].location
bpy.data.objects['P4'].rotation_euler = bpy.data.objects['Armature'].pose.bones['Bone'].rotation_euler
bpy.data.objects['P4'].scale = bpy.data.objects['Armature'].pose.bones['Bone'].scale
bpy.data.objects.new('Q4', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Q4'])
bpy.data.objects['Q4'].location = bpy.data.objects['Armature'].pose.bones['Bone'].rotation_quaternion[1:4]
bpy.data.objects['Q4'].scale.x = bpy.data.objects['Armature'].pose.bones['Bone'].rotation_quaternion[0]
bpy.data.objects['Q4'].rotation_euler = bpy.data.objects['Armature'].pose.bones['Bone'].rotation_axis_angle[1:4]
bpy.data.objects['Q4'].scale.y = bpy.data.objects['Armature'].pose.bones['Bone'].rotation_axis_angle[0]
bpy.data.objects.new('P5', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P5'])
bpy.data.objects['P5'].location = bpy.data.objects['Armature'].pose.bones['arm.L'].location
bpy.data.objects['P5'].rotation_euler = bpy.data.objects['Armature'].pose.bones['arm.L'].rotation_euler
bpy.data.objects['P5'].scale = bpy.data.objects['Armature'].pose.bones['arm.L'].scale
bpy.data.objects.new('Q5', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Q5'])
bpy.data.objects['Q5'].location = bpy.data.objects['Armature'].pose.bones['arm.L'].rotation_quaternion[1:4]
bpy.data.objects['Q5'].scale.x = bpy.data.objects['Armature'].pose.bones['arm.L'].rotation_quaternion[0]
bpy.data.objects['Q5'].rotation_euler = bpy.data.objects['Armature'].pose.bones['arm.L'].rotation_axis_angle[1:4]
bpy.data.objects['Q5'].scale.y = bpy.data.objects['Armature'].pose.bones['arm.L'].rotation_axis_angle[0]
bpy.data.objects.new('P6', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P6'])
bpy.data.objects['P6'].location = bpy.data.objects['Armature'].pose.bones['hand.L'].location
bpy.data.objects['P6'].rotation_euler = bpy.data.objects['Armature'].pose.bones['hand.L'].rotation_euler
bpy.data.objects['P6'].scale = bpy.data.objects['Armature'].pose.bones['hand.L'].scale
bpy.data.objects.new('Q6', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Q6'])
bpy.data.objects['Q6'].location = bpy.data.objects['Armature'].pose.bones['hand.L'].rotation_quaternion[1:4]
bpy.data.objects['Q6'].scale.x = bpy.data.objects['Armature'].pose.bones['hand.L'].rotation_quaternion[0]
bpy.data.objects['Q6'].rotation_euler = bpy.data.objects['Armature'].pose.bones['hand.L'].rotation_axis_angle[1:4]
bpy.data.objects['Q6'].scale.y = bpy.data.objects['Armature'].pose.bones['hand.L'].rotation_axis_angle[0]
bpy.data.objects.new('P7', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P7'])
bpy.data.objects['P7'].location = bpy.data.objects['Armature'].pose.bones['B1'].location
bpy.data.objects['P7'].rotation_euler = bpy.data.objects['Armature'].pose.bones['B1'].rotation_euler
bpy.data.objects['P7'].scale = bpy.data.objects['Armature'].pose.bones['B1'].scale
bpy.data.objects.new('Q7', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Q7'])
bpy.data.objects['Q7'].location = bpy.data.objects['Armature'].pose.bones['B1'].rotation_quaternion[1:4]
bpy.data.objects['Q7'].scale.x = bpy.data.objects['Armature'].pose.bones['B1'].rotation_quaternion[0]
bpy.data.objects['Q7'].rotation_euler = bpy.data.objects['Armature'].pose.bones['B1'].rotation_axis_angle[1:4]
bpy.data.objects['Q7'].scale.y = bpy.data.objects['Armature'].pose.bones['B1'].rotation_axis_angle[0]
bpy.data.objects.new('P8', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P8'])
bpy.data.objects['P8'].location = bpy.data.objects['Armature'].pose.bones['leg.L'].location
bpy.data.objects['P8'].rotation_euler = bpy.data.objects['Armature'].pose.bones['leg.L'].rotation_euler
bpy.data.objects['P8'].scale = bpy.data.objects['Armature'].pose.bones['leg.L'].scale
bpy.data.objects.new('Q8', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Q8'])
bpy.data.objects['Q8'].location = bpy.data.objects['Armature'].pose.bones['leg.L'].rotation_quaternion[1:4]
bpy.data.objects['Q8'].scale.x = bpy.data.objects['Armature'].pose.bones['leg.L'].rotation_quaternion[0]
bpy.data.objects['Q8'].rotation_euler = bpy.data.objects['Armature'].pose.bones['leg.L'].rotation_axis_angle[1:4]
bpy.data.objects['Q8'].scale.y = bpy.data.objects['Armature'].pose.bones['leg.L'].rotation_axis_angle[0]
bpy.data.objects.new('P9', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P9'])
bpy.data.objects['P9'].location = bpy.data.objects['Armature'].pose.bones['hand.R'].location
bpy.data.objects['P9'].rotation_euler = bpy.data.objects['Armature'].pose.bones['hand.R'].rotation_euler
bpy.data.objects['P9'].scale = bpy.data.objects['Armature'].pose.bones['hand.R'].scale
bpy.data.objects.new('Q9', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Q9'])
bpy.data.objects['Q9'].location = bpy.data.objects['Armature'].pose.bones['hand.R'].rotation_quaternion[1:4]
bpy.data.objects['Q9'].scale.x = bpy.data.objects['Armature'].pose.bones['hand.R'].rotation_quaternion[0]
bpy.data.objects['Q9'].rotation_euler = bpy.data.objects['Armature'].pose.bones['hand.R'].rotation_axis_angle[1:4]
bpy.data.objects['Q9'].scale.y = bpy.data.objects['Armature'].pose.bones['hand.R'].rotation_axis_angle[0]
bpy.data.armatures['Armature'].pose_position = 'REST'
bpy.ops.pose.select_all(action='DESELECT')
bpy.data.objects['Armature'].pose.bones['arm.R'].select = True
bpy.ops.pose.visual_transform_apply()
bpy.data.objects.new('P10', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P10'])
bpy.data.objects['P10'].location = bpy.data.objects['Armature'].pose.bones['arm.R'].location
bpy.data.objects['P10'].rotation_euler = bpy.data.objects['Armature'].pose.bones['arm.R'].rotation_euler
bpy.data.objects['P10'].scale = bpy.data.objects['Armature'].pose.bones['arm.R'].scale
bpy.data.objects.new('Q10', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Q10'])
bpy.data.objects['Q10'].location = bpy.data.objects['Armature'].pose.bones['arm.R'].rotation_quaternion[1:4]
bpy.data.objects['Q10'].scale.x = bpy.data.objects['Armature'].pose.bones['arm.R'].rotation_quaternion[0]
bpy.data.objects['Q10'].rotation_euler = bpy.data.objects['Armature'].pose.bones['arm.R'].rotation_axis_angle[1:4]
bpy.data.objects['Q10'].scale.y = bpy.data.objects['Armature'].pose.bones['arm.R'].rotation_axis_angle[0]
bpy.data.armatures['Armature'].pose_position = 'POSE'
bpy.ops.pose.select_all(action='DESELECT')
bpy.data.objects['Armature'].pose.bones['arm.L'].select = True
bpy.data.objects['Armature'].pose.bones['hand.L'].select = True
bpy.data.objects['Armature'].pose.bones['B1'].select = True
bpy.data.objects['Armature'].pose.bones['B1'].hide = True
bpy.ops.pose.user_transforms_clear()
bpy.data.objects.new('P11', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P11'])
bpy.data.objects['P11'].location = bpy.data.objects['Armature'].pose.bones['arm.L'].location
bpy.data.objects['P11'].rotation_euler = bpy.data.objects['Armature'].pose.bones['arm.L'].rotation_euler
bpy.data.objects['P11'].scale = bpy.data.objects['Armature'].pose.bones['arm.L'].scale
bpy.data.objects.new('Q11', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Q11'])
bpy.data.objects['Q11'].location = bpy.data.objects['Armature'].pose.bones['arm.L'].rotation_quaternion[1:4]
bpy.data.objects['Q11'].scale.x = bpy.data.objects['Armature'].pose.bones['arm.L'].rotation_quaternion[0]
bpy.data.objects['Q11'].rotation_euler = bpy.data.objects['Armature'].pose.bones['arm.L'].rotation_axis_angle[1:4]
bpy.data.objects['Q11'].scale.y = bpy.data.objects['Armature'].pose.bones['arm.L'].rotation_axis_angle[0]
bpy.data.objects.new('P12', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P12'])
bpy.data.objects['P12'].location = bpy.data.objects['Armature'].pose.bones['hand.L'].location
bpy.data.objects['P12'].rotation_euler = bpy.data.objects['Armature'].pose.bones['hand.L'].rotation_euler
bpy.data.objects['P12'].scale = bpy.data.objects['Armature'].pose.bones['hand.L'].scale
bpy.data.objects.new('Q12', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Q12'])
bpy.data.objects['Q12'].location = bpy.data.objects['Armature'].pose.bones['hand.L'].rotation_quaternion[1:4]
bpy.data.objects['Q12'].scale.x = bpy.data.objects['Armature'].pose.bones['hand.L'].rotation_quaternion[0]
bpy.data.objects['Q12'].rotation_euler = bpy.data.objects['Armature'].pose.bones['hand.L'].rotation_axis_angle[1:4]
bpy.data.objects['Q12'].scale.y = bpy.data.objects['Armature'].pose.bones['hand.L'].rotation_axis_angle[0]
bpy.data.objects.new('P13', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P13'])
bpy.data.objects['P13'].location = bpy.data.objects['Armature'].pose.bones['B1'].location
bpy.data.objects['P13'].rotation_euler = bpy.data.objects['Armature'].pose.bones['B1'].rotation_euler
bpy.data.objects['P13'].scale = bpy.data.objects['Armature'].pose.bones['B1'].scale
bpy.data.objects.new('Q13', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Q13'])
bpy.data.objects['Q13'].location = bpy.data.objects['Armature'].pose.bones['B1'].rotation_quaternion[1:4]
bpy.data.objects['Q13'].scale.x = bpy.data.objects['Armature'].pose.bones['B1'].rotation_quaternion[0]
bpy.data.objects['Q13'].rotation_euler = bpy.data.objects['Armature'].pose.bones['B1'].rotation_axis_angle[1:4]
bpy.data.objects['Q13'].scale.y = bpy.data.objects['Armature'].pose.bones['B1'].rotation_axis_angle[0]
bpy.data.objects.new('P14', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P14'])
bpy.data.objects['P14'].location = bpy.data.objects['Armature'].pose.bones['Bone'].location
bpy.data.objects['P14'].rotation_euler = bpy.data.objects['Armature'].pose.bones['Bone'].rotation_euler
bpy.data.objects['P14'].scale = bpy.data.objects['Armature'].pose.bones['Bone'].scale
bpy.data.objects.new('Q14', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Q14'])
bpy.data.objects['Q14'].location = bpy.data.objects['Armature'].pose.bones['Bone'].rotation_quaternion[1:4]
bpy.data.objects['Q14'].scale.x = bpy.data.objects['Armature'].pose.bones['Bone'].rotation_quaternion[0]
bpy.data.objects['Q14'].rotation_euler = bpy.data.objects['Armature'].pose.bones['Bone'].rotation_axis_angle[1:4]
bpy.data.objects['Q14'].scale.y = bpy.data.objects['Armature'].pose.bones['Bone'].rotation_axis_angle[0]
bpy.data.objects['Armature'].pose.bones['B1'].hide = False
bpy.ops.pose.user_transforms_clear(only_selected=False)
bpy.data.objects.new('P15', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P15'])
bpy.data.objects['P15'].location = bpy.data.objects['Armature'].pose.bones['Bone'].location
bpy.data.objects['P15'].rotation_euler = bpy.data.objects['Armature'].pose.bones['Bone'].rotation_euler
bpy.data.objects['P15'].scale = bpy.data.objects['Armature'].pose.bones['Bone'].scale
bpy.data.objects.new('Q15', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Q15'])
bpy.data.objects['Q15'].location = bpy.data.objects['Armature'].pose.bones['Bone'].rotation_quaternion[1:4]
bpy.data.objects['Q15'].scale.x = bpy.data.objects['Armature'].pose.bones['Bone'].rotation_quaternion[0]
bpy.data.objects['Q15'].rotation_euler = bpy.data.objects['Armature'].pose.bones['Bone'].rotation_axis_angle[1:4]
bpy.data.objects['Q15'].scale.y = bpy.data.objects['Armature'].pose.bones['Bone'].rotation_axis_angle[0]
bpy.data.objects.new('P16', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P16'])
bpy.data.objects['P16'].location = bpy.data.objects['Armature'].pose.bones['leg.L'].location
bpy.data.objects['P16'].rotation_euler = bpy.data.objects['Armature'].pose.bones['leg.L'].rotation_euler
bpy.data.objects['P16'].scale = bpy.data.objects['Armature'].pose.bones['leg.L'].scale
bpy.data.objects.new('Q16', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Q16'])
bpy.data.objects['Q16'].location = bpy.data.objects['Armature'].pose.bones['leg.L'].rotation_quaternion[1:4]
bpy.data.objects['Q16'].scale.x = bpy.data.objects['Armature'].pose.bones['leg.L'].rotation_quaternion[0]
bpy.data.objects['Q16'].rotation_euler = bpy.data.objects['Armature'].pose.bones['leg.L'].rotation_axis_angle[1:4]
bpy.data.objects['Q16'].scale.y = bpy.data.objects['Armature'].pose.bones['leg.L'].rotation_axis_angle[0]
bpy.data.objects['Armature'].pose.bones['arm.L'].location = (0, 0, 0)
bpy.data.objects['Armature'].pose.bones['arm.L'].rotation_quaternion = (1, 0, 0, 0)
bpy.data.objects['Armature'].pose.bones['arm.L'].keyframe_insert('location', frame=1)
bpy.data.objects['Armature'].pose.bones['arm.L'].keyframe_insert('rotation_quaternion', frame=1)
bpy.data.objects['Armature'].pose.bones['arm.L'].location = (1, 2, 3)
bpy.data.objects['Armature'].pose.bones['arm.L'].rotation_quaternion = (0, 1, 0, 0)
bpy.data.objects['Armature'].pose.bones['arm.L'].keyframe_insert('location', frame=11)
bpy.data.objects['Armature'].pose.bones['arm.L'].keyframe_insert('rotation_quaternion', frame=11)
bpy.data.objects['Armature'].pose.bones['leg.L'].rotation_mode = 'XYZ'
bpy.data.objects['Armature'].pose.bones['leg.L'].rotation_euler = (0.5, 0, 0)
bpy.data.objects['Armature'].pose.bones['leg.L'].keyframe_insert('rotation_euler', index=0, frame=1)
bpy.data.objects['Armature'].pose.bones['leg.L'].rotation_euler = (1.5, 0, 0)
bpy.data.objects['Armature'].pose.bones['leg.L'].keyframe_insert('rotation_euler', index=0, frame=11)
bpy.context.scene.frame_set(4)
bpy.data.objects['Armature'].pose.bones['arm.L'].location = (5, 5, 5)
bpy.data.objects['Armature'].pose.bones['arm.L'].rotation_quaternion = (0.5, 0.5, 0.5, 0.5)
bpy.data.objects['Armature'].pose.bones['arm.L'].scale = (2, 2, 2)
bpy.data.objects['Armature'].pose.bones['leg.L'].location = (1, 1, 1)
bpy.data.objects['Armature'].pose.bones['leg.L'].rotation_euler = (0.1, 0.2, 0.3)
bpy.data.objects['Armature'].pose.bones['hand.L'].location = (0.3, 0.3, 0.3)
bpy.ops.pose.select_all(action='DESELECT')
bpy.data.objects['Armature'].pose.bones['leg.L'].select = True
bpy.data.objects['Armature'].pose.bones['hand.L'].select = True
bpy.ops.pose.user_transforms_clear()
bpy.data.objects.new('P17', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P17'])
bpy.data.objects['P17'].location = bpy.data.objects['Armature'].pose.bones['arm.L'].location
bpy.data.objects['P17'].rotation_euler = bpy.data.objects['Armature'].pose.bones['arm.L'].rotation_euler
bpy.data.objects['P17'].scale = bpy.data.objects['Armature'].pose.bones['arm.L'].scale
bpy.data.objects.new('Q17', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Q17'])
bpy.data.objects['Q17'].location = bpy.data.objects['Armature'].pose.bones['arm.L'].rotation_quaternion[1:4]
bpy.data.objects['Q17'].scale.x = bpy.data.objects['Armature'].pose.bones['arm.L'].rotation_quaternion[0]
bpy.data.objects['Q17'].rotation_euler = bpy.data.objects['Armature'].pose.bones['arm.L'].rotation_axis_angle[1:4]
bpy.data.objects['Q17'].scale.y = bpy.data.objects['Armature'].pose.bones['arm.L'].rotation_axis_angle[0]
bpy.data.objects.new('P18', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P18'])
bpy.data.objects['P18'].location = bpy.data.objects['Armature'].pose.bones['leg.L'].location
bpy.data.objects['P18'].rotation_euler = bpy.data.objects['Armature'].pose.bones['leg.L'].rotation_euler
bpy.data.objects['P18'].scale = bpy.data.objects['Armature'].pose.bones['leg.L'].scale
bpy.data.objects.new('Q18', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Q18'])
bpy.data.objects['Q18'].location = bpy.data.objects['Armature'].pose.bones['leg.L'].rotation_quaternion[1:4]
bpy.data.objects['Q18'].scale.x = bpy.data.objects['Armature'].pose.bones['leg.L'].rotation_quaternion[0]
bpy.data.objects['Q18'].rotation_euler = bpy.data.objects['Armature'].pose.bones['leg.L'].rotation_axis_angle[1:4]
bpy.data.objects['Q18'].scale.y = bpy.data.objects['Armature'].pose.bones['leg.L'].rotation_axis_angle[0]
bpy.data.objects.new('P19', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P19'])
bpy.data.objects['P19'].location = bpy.data.objects['Armature'].pose.bones['hand.L'].location
bpy.data.objects['P19'].rotation_euler = bpy.data.objects['Armature'].pose.bones['hand.L'].rotation_euler
bpy.data.objects['P19'].scale = bpy.data.objects['Armature'].pose.bones['hand.L'].scale
bpy.data.objects.new('Q19', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Q19'])
bpy.data.objects['Q19'].location = bpy.data.objects['Armature'].pose.bones['hand.L'].rotation_quaternion[1:4]
bpy.data.objects['Q19'].scale.x = bpy.data.objects['Armature'].pose.bones['hand.L'].rotation_quaternion[0]
bpy.data.objects['Q19'].rotation_euler = bpy.data.objects['Armature'].pose.bones['hand.L'].rotation_axis_angle[1:4]
bpy.data.objects['Q19'].scale.y = bpy.data.objects['Armature'].pose.bones['hand.L'].rotation_axis_angle[0]
bpy.ops.pose.user_transforms_clear(only_selected=False)
bpy.data.objects.new('P20', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P20'])
bpy.data.objects['P20'].location = bpy.data.objects['Armature'].pose.bones['arm.L'].location
bpy.data.objects['P20'].rotation_euler = bpy.data.objects['Armature'].pose.bones['arm.L'].rotation_euler
bpy.data.objects['P20'].scale = bpy.data.objects['Armature'].pose.bones['arm.L'].scale
bpy.data.objects.new('Q20', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Q20'])
bpy.data.objects['Q20'].location = bpy.data.objects['Armature'].pose.bones['arm.L'].rotation_quaternion[1:4]
bpy.data.objects['Q20'].scale.x = bpy.data.objects['Armature'].pose.bones['arm.L'].rotation_quaternion[0]
bpy.data.objects['Q20'].rotation_euler = bpy.data.objects['Armature'].pose.bones['arm.L'].rotation_axis_angle[1:4]
bpy.data.objects['Q20'].scale.y = bpy.data.objects['Armature'].pose.bones['arm.L'].rotation_axis_angle[0]
bpy.data.objects.new('P21', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P21'])
bpy.data.objects['P21'].location = bpy.data.objects['Armature'].pose.bones['leg.L'].location
bpy.data.objects['P21'].rotation_euler = bpy.data.objects['Armature'].pose.bones['leg.L'].rotation_euler
bpy.data.objects['P21'].scale = bpy.data.objects['Armature'].pose.bones['leg.L'].scale
bpy.data.objects.new('Q21', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Q21'])
bpy.data.objects['Q21'].location = bpy.data.objects['Armature'].pose.bones['leg.L'].rotation_quaternion[1:4]
bpy.data.objects['Q21'].scale.x = bpy.data.objects['Armature'].pose.bones['leg.L'].rotation_quaternion[0]
bpy.data.objects['Q21'].rotation_euler = bpy.data.objects['Armature'].pose.bones['leg.L'].rotation_axis_angle[1:4]
bpy.data.objects['Q21'].scale.y = bpy.data.objects['Armature'].pose.bones['leg.L'].rotation_axis_angle[0]
bpy.data.objects.new('P22', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P22'])
bpy.data.objects['P22'].location = bpy.data.objects['Armature'].pose.bones['hand.L'].location
bpy.data.objects['P22'].rotation_euler = bpy.data.objects['Armature'].pose.bones['hand.L'].rotation_euler
bpy.data.objects['P22'].scale = bpy.data.objects['Armature'].pose.bones['hand.L'].scale
bpy.data.objects.new('Q22', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Q22'])
bpy.data.objects['Q22'].location = bpy.data.objects['Armature'].pose.bones['hand.L'].rotation_quaternion[1:4]
bpy.data.objects['Q22'].scale.x = bpy.data.objects['Armature'].pose.bones['hand.L'].rotation_quaternion[0]
bpy.data.objects['Q22'].rotation_euler = bpy.data.objects['Armature'].pose.bones['hand.L'].rotation_axis_angle[1:4]
bpy.data.objects['Q22'].scale.y = bpy.data.objects['Armature'].pose.bones['hand.L'].rotation_axis_angle[0]
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.pose.visual_transform_apply()
bpy.ops.pose.quaternions_flip()
bpy.ops.pose.user_transforms_clear()
