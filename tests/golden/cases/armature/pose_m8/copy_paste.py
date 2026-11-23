# WP 8.3.11, pose.copy / pose.paste with Blender's copy buffer: copy deselects hidden bones
# in the scene, the buffer holds the channels selected at copy time (a later scene change does
# not reach it), paste converts the rotation to each target's mode (quaternion, Euler orders,
# axis-angle; an axis-angle source onto a quaternion bone takes the target's angle), flipped
# pastes onto the X-mirrored names with location X and Euler Y/Z negated (axis-angle and
# quaternion through XYZ Euler), selected_mask pastes only onto selected bones (hidden selected
# ones too) unless none is selected, bones missing from the target are skipped, a second copy
# replaces the buffer, and the Pose Mode poll. P<n>/Q<n> hold location, Euler, scale,
# quaternion and axis-angle of one bone, F<n> a flag.
# golden: startup=factory compare=objects tier=T0 tol=1e-5
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
bpy.data.objects['Armature'].pose.bones['arm.L'].location = (0.1, 0.2, 0.3)
bpy.data.objects['Armature'].pose.bones['arm.L'].rotation_quaternion = (0.9, 0.1, 0.3, 0.2)
bpy.data.objects['Armature'].pose.bones['arm.L'].scale = (1, 2, 1.5)
bpy.data.objects['Armature'].pose.bones['arm.R'].rotation_mode = 'XYZ'
bpy.data.objects['Armature'].pose.bones['arm.R'].location = (0.5, 0.5, 0.5)
bpy.data.objects['Armature'].pose.bones['arm.R'].rotation_euler = (0.3, 0.2, 0.1)
bpy.data.objects['Armature'].pose.bones['hand.L'].rotation_mode = 'AXIS_ANGLE'
bpy.data.objects['Armature'].pose.bones['hand.L'].location = (0.2, 0, 0)
bpy.data.objects['Armature'].pose.bones['hand.L'].rotation_axis_angle = (0.7, 0.2, 0.5, 0.3)
bpy.data.objects['Armature'].pose.bones['hand.R'].rotation_axis_angle = (1.3, 0, 1, 0)
bpy.data.objects['Armature'].pose.bones['B1'].rotation_mode = 'ZXY'
bpy.data.objects['Armature'].pose.bones['B1'].location = (0.3, -0.2, 0.1)
bpy.data.objects['Armature'].pose.bones['B1'].rotation_euler = (0.4, 0.5, 0.6)
bpy.data.objects['Armature'].pose.bones['leg.L'].rotation_mode = 'YZX'
bpy.data.objects['Armature'].pose.bones['leg.L'].rotation_euler = (0.1, -0.7, 0.2)
bpy.data.objects['Armature'].pose.bones['leg.L'].scale = (0.5, 0.5, 2)
bpy.data.objects['Armature'].pose.bones['leg.R'].rotation_mode = 'AXIS_ANGLE'
bpy.data.objects['Armature'].pose.bones['leg.R'].rotation_axis_angle = (0.4, 1, 0, 0)
bpy.data.objects['Armature'].pose.bones['Bone'].location = (0, 0.4, 0)
bpy.data.objects['Armature'].pose.bones['Bone'].rotation_quaternion = (0.7, 0.7, 0.1, 0)
bpy.ops.pose.select_all(action='DESELECT')
bpy.data.objects['Armature'].pose.bones['arm.L'].select = True
bpy.data.objects['Armature'].pose.bones['hand.L'].select = True
bpy.data.objects['Armature'].pose.bones['B1'].select = True
bpy.data.objects['Armature'].pose.bones['leg.L'].select = True
bpy.data.objects['Armature'].pose.bones['leg.R'].select = True
bpy.data.objects['Armature'].pose.bones['Bone'].select = True
bpy.data.armatures['Armature'].bones.active = bpy.data.armatures['Armature'].bones['arm.L']
bpy.data.objects['Armature'].pose.bones['B1'].hide = True
bpy.data.objects['Armature'].pose.bones['B1'].select = True
bpy.ops.pose.copy()
bpy.data.objects.new('F0', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['F0'])
bpy.data.objects['F0'].location.x = bpy.data.objects['Armature'].pose.bones['B1'].select
bpy.data.objects['Armature'].pose.bones['B1'].hide = False
bpy.data.objects['Armature'].pose.bones['Bone'].location = (9, 9, 9)
bpy.data.objects['Armature'].pose.bones['Bone'].scale = (3, 3, 3)
bpy.data.objects['Armature'].pose.bones['arm.L'].location = (9, 9, 9)
bpy.data.objects['Armature'].pose.bones['arm.L'].scale = (3, 3, 3)
bpy.data.objects['Armature'].pose.bones['arm.R'].location = (9, 9, 9)
bpy.data.objects['Armature'].pose.bones['arm.R'].scale = (3, 3, 3)
bpy.data.objects['Armature'].pose.bones['hand.L'].location = (9, 9, 9)
bpy.data.objects['Armature'].pose.bones['hand.L'].scale = (3, 3, 3)
bpy.data.objects['Armature'].pose.bones['hand.R'].location = (9, 9, 9)
bpy.data.objects['Armature'].pose.bones['hand.R'].scale = (3, 3, 3)
bpy.data.objects['Armature'].pose.bones['B1'].location = (9, 9, 9)
bpy.data.objects['Armature'].pose.bones['B1'].scale = (3, 3, 3)
bpy.data.objects['Armature'].pose.bones['leg.L'].location = (9, 9, 9)
bpy.data.objects['Armature'].pose.bones['leg.L'].scale = (3, 3, 3)
bpy.data.objects['Armature'].pose.bones['leg.R'].location = (9, 9, 9)
bpy.data.objects['Armature'].pose.bones['leg.R'].scale = (3, 3, 3)
bpy.ops.pose.select_all(action='DESELECT')
bpy.ops.pose.paste()
bpy.data.objects.new('P1', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P1'])
bpy.data.objects['P1'].location = bpy.data.objects['Armature'].pose.bones['Bone'].location
bpy.data.objects['P1'].rotation_euler = bpy.data.objects['Armature'].pose.bones['Bone'].rotation_euler
bpy.data.objects['P1'].scale = bpy.data.objects['Armature'].pose.bones['Bone'].scale
bpy.data.objects.new('Q1', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Q1'])
bpy.data.objects['Q1'].location = bpy.data.objects['Armature'].pose.bones['Bone'].rotation_quaternion[1:4]
bpy.data.objects['Q1'].scale.x = bpy.data.objects['Armature'].pose.bones['Bone'].rotation_quaternion[0]
bpy.data.objects['Q1'].rotation_euler = bpy.data.objects['Armature'].pose.bones['Bone'].rotation_axis_angle[1:4]
bpy.data.objects['Q1'].scale.y = bpy.data.objects['Armature'].pose.bones['Bone'].rotation_axis_angle[0]
bpy.data.objects.new('P2', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P2'])
bpy.data.objects['P2'].location = bpy.data.objects['Armature'].pose.bones['arm.L'].location
bpy.data.objects['P2'].rotation_euler = bpy.data.objects['Armature'].pose.bones['arm.L'].rotation_euler
bpy.data.objects['P2'].scale = bpy.data.objects['Armature'].pose.bones['arm.L'].scale
bpy.data.objects.new('Q2', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Q2'])
bpy.data.objects['Q2'].location = bpy.data.objects['Armature'].pose.bones['arm.L'].rotation_quaternion[1:4]
bpy.data.objects['Q2'].scale.x = bpy.data.objects['Armature'].pose.bones['arm.L'].rotation_quaternion[0]
bpy.data.objects['Q2'].rotation_euler = bpy.data.objects['Armature'].pose.bones['arm.L'].rotation_axis_angle[1:4]
bpy.data.objects['Q2'].scale.y = bpy.data.objects['Armature'].pose.bones['arm.L'].rotation_axis_angle[0]
bpy.data.objects.new('P3', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P3'])
bpy.data.objects['P3'].location = bpy.data.objects['Armature'].pose.bones['arm.R'].location
bpy.data.objects['P3'].rotation_euler = bpy.data.objects['Armature'].pose.bones['arm.R'].rotation_euler
bpy.data.objects['P3'].scale = bpy.data.objects['Armature'].pose.bones['arm.R'].scale
bpy.data.objects.new('Q3', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Q3'])
bpy.data.objects['Q3'].location = bpy.data.objects['Armature'].pose.bones['arm.R'].rotation_quaternion[1:4]
bpy.data.objects['Q3'].scale.x = bpy.data.objects['Armature'].pose.bones['arm.R'].rotation_quaternion[0]
bpy.data.objects['Q3'].rotation_euler = bpy.data.objects['Armature'].pose.bones['arm.R'].rotation_axis_angle[1:4]
bpy.data.objects['Q3'].scale.y = bpy.data.objects['Armature'].pose.bones['arm.R'].rotation_axis_angle[0]
bpy.data.objects.new('P4', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P4'])
bpy.data.objects['P4'].location = bpy.data.objects['Armature'].pose.bones['hand.L'].location
bpy.data.objects['P4'].rotation_euler = bpy.data.objects['Armature'].pose.bones['hand.L'].rotation_euler
bpy.data.objects['P4'].scale = bpy.data.objects['Armature'].pose.bones['hand.L'].scale
bpy.data.objects.new('Q4', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Q4'])
bpy.data.objects['Q4'].location = bpy.data.objects['Armature'].pose.bones['hand.L'].rotation_quaternion[1:4]
bpy.data.objects['Q4'].scale.x = bpy.data.objects['Armature'].pose.bones['hand.L'].rotation_quaternion[0]
bpy.data.objects['Q4'].rotation_euler = bpy.data.objects['Armature'].pose.bones['hand.L'].rotation_axis_angle[1:4]
bpy.data.objects['Q4'].scale.y = bpy.data.objects['Armature'].pose.bones['hand.L'].rotation_axis_angle[0]
bpy.data.objects.new('P5', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P5'])
bpy.data.objects['P5'].location = bpy.data.objects['Armature'].pose.bones['hand.R'].location
bpy.data.objects['P5'].rotation_euler = bpy.data.objects['Armature'].pose.bones['hand.R'].rotation_euler
bpy.data.objects['P5'].scale = bpy.data.objects['Armature'].pose.bones['hand.R'].scale
bpy.data.objects.new('Q5', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Q5'])
bpy.data.objects['Q5'].location = bpy.data.objects['Armature'].pose.bones['hand.R'].rotation_quaternion[1:4]
bpy.data.objects['Q5'].scale.x = bpy.data.objects['Armature'].pose.bones['hand.R'].rotation_quaternion[0]
bpy.data.objects['Q5'].rotation_euler = bpy.data.objects['Armature'].pose.bones['hand.R'].rotation_axis_angle[1:4]
bpy.data.objects['Q5'].scale.y = bpy.data.objects['Armature'].pose.bones['hand.R'].rotation_axis_angle[0]
bpy.data.objects.new('P6', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P6'])
bpy.data.objects['P6'].location = bpy.data.objects['Armature'].pose.bones['B1'].location
bpy.data.objects['P6'].rotation_euler = bpy.data.objects['Armature'].pose.bones['B1'].rotation_euler
bpy.data.objects['P6'].scale = bpy.data.objects['Armature'].pose.bones['B1'].scale
bpy.data.objects.new('Q6', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Q6'])
bpy.data.objects['Q6'].location = bpy.data.objects['Armature'].pose.bones['B1'].rotation_quaternion[1:4]
bpy.data.objects['Q6'].scale.x = bpy.data.objects['Armature'].pose.bones['B1'].rotation_quaternion[0]
bpy.data.objects['Q6'].rotation_euler = bpy.data.objects['Armature'].pose.bones['B1'].rotation_axis_angle[1:4]
bpy.data.objects['Q6'].scale.y = bpy.data.objects['Armature'].pose.bones['B1'].rotation_axis_angle[0]
bpy.data.objects.new('P7', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P7'])
bpy.data.objects['P7'].location = bpy.data.objects['Armature'].pose.bones['leg.L'].location
bpy.data.objects['P7'].rotation_euler = bpy.data.objects['Armature'].pose.bones['leg.L'].rotation_euler
bpy.data.objects['P7'].scale = bpy.data.objects['Armature'].pose.bones['leg.L'].scale
bpy.data.objects.new('Q7', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Q7'])
bpy.data.objects['Q7'].location = bpy.data.objects['Armature'].pose.bones['leg.L'].rotation_quaternion[1:4]
bpy.data.objects['Q7'].scale.x = bpy.data.objects['Armature'].pose.bones['leg.L'].rotation_quaternion[0]
bpy.data.objects['Q7'].rotation_euler = bpy.data.objects['Armature'].pose.bones['leg.L'].rotation_axis_angle[1:4]
bpy.data.objects['Q7'].scale.y = bpy.data.objects['Armature'].pose.bones['leg.L'].rotation_axis_angle[0]
bpy.data.objects.new('P8', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P8'])
bpy.data.objects['P8'].location = bpy.data.objects['Armature'].pose.bones['leg.R'].location
bpy.data.objects['P8'].rotation_euler = bpy.data.objects['Armature'].pose.bones['leg.R'].rotation_euler
bpy.data.objects['P8'].scale = bpy.data.objects['Armature'].pose.bones['leg.R'].scale
bpy.data.objects.new('Q8', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Q8'])
bpy.data.objects['Q8'].location = bpy.data.objects['Armature'].pose.bones['leg.R'].rotation_quaternion[1:4]
bpy.data.objects['Q8'].scale.x = bpy.data.objects['Armature'].pose.bones['leg.R'].rotation_quaternion[0]
bpy.data.objects['Q8'].rotation_euler = bpy.data.objects['Armature'].pose.bones['leg.R'].rotation_axis_angle[1:4]
bpy.data.objects['Q8'].scale.y = bpy.data.objects['Armature'].pose.bones['leg.R'].rotation_axis_angle[0]
bpy.ops.pose.paste(flipped=True)
bpy.data.objects.new('P9', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P9'])
bpy.data.objects['P9'].location = bpy.data.objects['Armature'].pose.bones['Bone'].location
bpy.data.objects['P9'].rotation_euler = bpy.data.objects['Armature'].pose.bones['Bone'].rotation_euler
bpy.data.objects['P9'].scale = bpy.data.objects['Armature'].pose.bones['Bone'].scale
bpy.data.objects.new('Q9', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Q9'])
bpy.data.objects['Q9'].location = bpy.data.objects['Armature'].pose.bones['Bone'].rotation_quaternion[1:4]
bpy.data.objects['Q9'].scale.x = bpy.data.objects['Armature'].pose.bones['Bone'].rotation_quaternion[0]
bpy.data.objects['Q9'].rotation_euler = bpy.data.objects['Armature'].pose.bones['Bone'].rotation_axis_angle[1:4]
bpy.data.objects['Q9'].scale.y = bpy.data.objects['Armature'].pose.bones['Bone'].rotation_axis_angle[0]
bpy.data.objects.new('P10', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P10'])
bpy.data.objects['P10'].location = bpy.data.objects['Armature'].pose.bones['arm.L'].location
bpy.data.objects['P10'].rotation_euler = bpy.data.objects['Armature'].pose.bones['arm.L'].rotation_euler
bpy.data.objects['P10'].scale = bpy.data.objects['Armature'].pose.bones['arm.L'].scale
bpy.data.objects.new('Q10', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Q10'])
bpy.data.objects['Q10'].location = bpy.data.objects['Armature'].pose.bones['arm.L'].rotation_quaternion[1:4]
bpy.data.objects['Q10'].scale.x = bpy.data.objects['Armature'].pose.bones['arm.L'].rotation_quaternion[0]
bpy.data.objects['Q10'].rotation_euler = bpy.data.objects['Armature'].pose.bones['arm.L'].rotation_axis_angle[1:4]
bpy.data.objects['Q10'].scale.y = bpy.data.objects['Armature'].pose.bones['arm.L'].rotation_axis_angle[0]
bpy.data.objects.new('P11', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P11'])
bpy.data.objects['P11'].location = bpy.data.objects['Armature'].pose.bones['arm.R'].location
bpy.data.objects['P11'].rotation_euler = bpy.data.objects['Armature'].pose.bones['arm.R'].rotation_euler
bpy.data.objects['P11'].scale = bpy.data.objects['Armature'].pose.bones['arm.R'].scale
bpy.data.objects.new('Q11', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Q11'])
bpy.data.objects['Q11'].location = bpy.data.objects['Armature'].pose.bones['arm.R'].rotation_quaternion[1:4]
bpy.data.objects['Q11'].scale.x = bpy.data.objects['Armature'].pose.bones['arm.R'].rotation_quaternion[0]
bpy.data.objects['Q11'].rotation_euler = bpy.data.objects['Armature'].pose.bones['arm.R'].rotation_axis_angle[1:4]
bpy.data.objects['Q11'].scale.y = bpy.data.objects['Armature'].pose.bones['arm.R'].rotation_axis_angle[0]
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
bpy.data.objects['P13'].location = bpy.data.objects['Armature'].pose.bones['hand.R'].location
bpy.data.objects['P13'].rotation_euler = bpy.data.objects['Armature'].pose.bones['hand.R'].rotation_euler
bpy.data.objects['P13'].scale = bpy.data.objects['Armature'].pose.bones['hand.R'].scale
bpy.data.objects.new('Q13', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Q13'])
bpy.data.objects['Q13'].location = bpy.data.objects['Armature'].pose.bones['hand.R'].rotation_quaternion[1:4]
bpy.data.objects['Q13'].scale.x = bpy.data.objects['Armature'].pose.bones['hand.R'].rotation_quaternion[0]
bpy.data.objects['Q13'].rotation_euler = bpy.data.objects['Armature'].pose.bones['hand.R'].rotation_axis_angle[1:4]
bpy.data.objects['Q13'].scale.y = bpy.data.objects['Armature'].pose.bones['hand.R'].rotation_axis_angle[0]
bpy.data.objects.new('P14', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P14'])
bpy.data.objects['P14'].location = bpy.data.objects['Armature'].pose.bones['B1'].location
bpy.data.objects['P14'].rotation_euler = bpy.data.objects['Armature'].pose.bones['B1'].rotation_euler
bpy.data.objects['P14'].scale = bpy.data.objects['Armature'].pose.bones['B1'].scale
bpy.data.objects.new('Q14', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Q14'])
bpy.data.objects['Q14'].location = bpy.data.objects['Armature'].pose.bones['B1'].rotation_quaternion[1:4]
bpy.data.objects['Q14'].scale.x = bpy.data.objects['Armature'].pose.bones['B1'].rotation_quaternion[0]
bpy.data.objects['Q14'].rotation_euler = bpy.data.objects['Armature'].pose.bones['B1'].rotation_axis_angle[1:4]
bpy.data.objects['Q14'].scale.y = bpy.data.objects['Armature'].pose.bones['B1'].rotation_axis_angle[0]
bpy.data.objects.new('P15', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P15'])
bpy.data.objects['P15'].location = bpy.data.objects['Armature'].pose.bones['leg.L'].location
bpy.data.objects['P15'].rotation_euler = bpy.data.objects['Armature'].pose.bones['leg.L'].rotation_euler
bpy.data.objects['P15'].scale = bpy.data.objects['Armature'].pose.bones['leg.L'].scale
bpy.data.objects.new('Q15', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Q15'])
bpy.data.objects['Q15'].location = bpy.data.objects['Armature'].pose.bones['leg.L'].rotation_quaternion[1:4]
bpy.data.objects['Q15'].scale.x = bpy.data.objects['Armature'].pose.bones['leg.L'].rotation_quaternion[0]
bpy.data.objects['Q15'].rotation_euler = bpy.data.objects['Armature'].pose.bones['leg.L'].rotation_axis_angle[1:4]
bpy.data.objects['Q15'].scale.y = bpy.data.objects['Armature'].pose.bones['leg.L'].rotation_axis_angle[0]
bpy.data.objects.new('P16', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P16'])
bpy.data.objects['P16'].location = bpy.data.objects['Armature'].pose.bones['leg.R'].location
bpy.data.objects['P16'].rotation_euler = bpy.data.objects['Armature'].pose.bones['leg.R'].rotation_euler
bpy.data.objects['P16'].scale = bpy.data.objects['Armature'].pose.bones['leg.R'].scale
bpy.data.objects.new('Q16', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Q16'])
bpy.data.objects['Q16'].location = bpy.data.objects['Armature'].pose.bones['leg.R'].rotation_quaternion[1:4]
bpy.data.objects['Q16'].scale.x = bpy.data.objects['Armature'].pose.bones['leg.R'].rotation_quaternion[0]
bpy.data.objects['Q16'].rotation_euler = bpy.data.objects['Armature'].pose.bones['leg.R'].rotation_axis_angle[1:4]
bpy.data.objects['Q16'].scale.y = bpy.data.objects['Armature'].pose.bones['leg.R'].rotation_axis_angle[0]
bpy.data.objects['Armature'].pose.bones['arm.L'].location = (7, 7, 7)
bpy.data.objects['Armature'].pose.bones['hand.L'].location = (7, 7, 7)
bpy.ops.pose.select_all(action='DESELECT')
bpy.data.objects['Armature'].pose.bones['arm.L'].select = True
bpy.data.objects['Armature'].pose.bones['leg.R'].select = True
bpy.data.objects['Armature'].pose.bones['leg.R'].hide = True
bpy.ops.pose.paste(selected_mask=True)
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
bpy.data.objects['P18'].location = bpy.data.objects['Armature'].pose.bones['hand.L'].location
bpy.data.objects['P18'].rotation_euler = bpy.data.objects['Armature'].pose.bones['hand.L'].rotation_euler
bpy.data.objects['P18'].scale = bpy.data.objects['Armature'].pose.bones['hand.L'].scale
bpy.data.objects.new('Q18', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Q18'])
bpy.data.objects['Q18'].location = bpy.data.objects['Armature'].pose.bones['hand.L'].rotation_quaternion[1:4]
bpy.data.objects['Q18'].scale.x = bpy.data.objects['Armature'].pose.bones['hand.L'].rotation_quaternion[0]
bpy.data.objects['Q18'].rotation_euler = bpy.data.objects['Armature'].pose.bones['hand.L'].rotation_axis_angle[1:4]
bpy.data.objects['Q18'].scale.y = bpy.data.objects['Armature'].pose.bones['hand.L'].rotation_axis_angle[0]
bpy.data.objects.new('P19', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P19'])
bpy.data.objects['P19'].location = bpy.data.objects['Armature'].pose.bones['leg.R'].location
bpy.data.objects['P19'].rotation_euler = bpy.data.objects['Armature'].pose.bones['leg.R'].rotation_euler
bpy.data.objects['P19'].scale = bpy.data.objects['Armature'].pose.bones['leg.R'].scale
bpy.data.objects.new('Q19', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Q19'])
bpy.data.objects['Q19'].location = bpy.data.objects['Armature'].pose.bones['leg.R'].rotation_quaternion[1:4]
bpy.data.objects['Q19'].scale.x = bpy.data.objects['Armature'].pose.bones['leg.R'].rotation_quaternion[0]
bpy.data.objects['Q19'].rotation_euler = bpy.data.objects['Armature'].pose.bones['leg.R'].rotation_axis_angle[1:4]
bpy.data.objects['Q19'].scale.y = bpy.data.objects['Armature'].pose.bones['leg.R'].rotation_axis_angle[0]
bpy.data.objects['Armature'].pose.bones['leg.R'].hide = False
bpy.ops.pose.paste(selected_mask=True, flipped=True)
bpy.data.objects.new('P20', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P20'])
bpy.data.objects['P20'].location = bpy.data.objects['Armature'].pose.bones['arm.R'].location
bpy.data.objects['P20'].rotation_euler = bpy.data.objects['Armature'].pose.bones['arm.R'].rotation_euler
bpy.data.objects['P20'].scale = bpy.data.objects['Armature'].pose.bones['arm.R'].scale
bpy.data.objects.new('Q20', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Q20'])
bpy.data.objects['Q20'].location = bpy.data.objects['Armature'].pose.bones['arm.R'].rotation_quaternion[1:4]
bpy.data.objects['Q20'].scale.x = bpy.data.objects['Armature'].pose.bones['arm.R'].rotation_quaternion[0]
bpy.data.objects['Q20'].rotation_euler = bpy.data.objects['Armature'].pose.bones['arm.R'].rotation_axis_angle[1:4]
bpy.data.objects['Q20'].scale.y = bpy.data.objects['Armature'].pose.bones['arm.R'].rotation_axis_angle[0]
bpy.data.objects.new('P21', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P21'])
bpy.data.objects['P21'].location = bpy.data.objects['Armature'].pose.bones['hand.R'].location
bpy.data.objects['P21'].rotation_euler = bpy.data.objects['Armature'].pose.bones['hand.R'].rotation_euler
bpy.data.objects['P21'].scale = bpy.data.objects['Armature'].pose.bones['hand.R'].scale
bpy.data.objects.new('Q21', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Q21'])
bpy.data.objects['Q21'].location = bpy.data.objects['Armature'].pose.bones['hand.R'].rotation_quaternion[1:4]
bpy.data.objects['Q21'].scale.x = bpy.data.objects['Armature'].pose.bones['hand.R'].rotation_quaternion[0]
bpy.data.objects['Q21'].rotation_euler = bpy.data.objects['Armature'].pose.bones['hand.R'].rotation_axis_angle[1:4]
bpy.data.objects['Q21'].scale.y = bpy.data.objects['Armature'].pose.bones['hand.R'].rotation_axis_angle[0]
bpy.ops.pose.select_all(action='DESELECT')
bpy.ops.pose.paste(selected_mask=True)
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
bpy.ops.pose.select_all(action='DESELECT')
bpy.data.objects['Armature'].pose.bones['hand.R'].select = True
bpy.ops.pose.copy()
bpy.data.objects['Armature'].pose.bones['hand.R'].location = (4, 4, 4)
bpy.data.armatures['Armature'].bones['hand.R'].name = 'claw.R'
bpy.ops.pose.paste()
bpy.data.objects.new('P23', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P23'])
bpy.data.objects['P23'].location = bpy.data.objects['Armature'].pose.bones['claw.R'].location
bpy.data.objects['P23'].rotation_euler = bpy.data.objects['Armature'].pose.bones['claw.R'].rotation_euler
bpy.data.objects['P23'].scale = bpy.data.objects['Armature'].pose.bones['claw.R'].scale
bpy.data.objects.new('Q23', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Q23'])
bpy.data.objects['Q23'].location = bpy.data.objects['Armature'].pose.bones['claw.R'].rotation_quaternion[1:4]
bpy.data.objects['Q23'].scale.x = bpy.data.objects['Armature'].pose.bones['claw.R'].rotation_quaternion[0]
bpy.data.objects['Q23'].rotation_euler = bpy.data.objects['Armature'].pose.bones['claw.R'].rotation_axis_angle[1:4]
bpy.data.objects['Q23'].scale.y = bpy.data.objects['Armature'].pose.bones['claw.R'].rotation_axis_angle[0]
bpy.ops.pose.paste(flipped=True)
bpy.data.objects.new('P24', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P24'])
bpy.data.objects['P24'].location = bpy.data.objects['Armature'].pose.bones['hand.L'].location
bpy.data.objects['P24'].rotation_euler = bpy.data.objects['Armature'].pose.bones['hand.L'].rotation_euler
bpy.data.objects['P24'].scale = bpy.data.objects['Armature'].pose.bones['hand.L'].scale
bpy.data.objects.new('Q24', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Q24'])
bpy.data.objects['Q24'].location = bpy.data.objects['Armature'].pose.bones['hand.L'].rotation_quaternion[1:4]
bpy.data.objects['Q24'].scale.x = bpy.data.objects['Armature'].pose.bones['hand.L'].rotation_quaternion[0]
bpy.data.objects['Q24'].rotation_euler = bpy.data.objects['Armature'].pose.bones['hand.L'].rotation_axis_angle[1:4]
bpy.data.objects['Q24'].scale.y = bpy.data.objects['Armature'].pose.bones['hand.L'].rotation_axis_angle[0]
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.pose.copy()
bpy.ops.pose.paste()
