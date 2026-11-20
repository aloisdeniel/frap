# WP 6.2, BKE_pose_where_is: pose matrices of a five-bone rig (a connected chain, a hinge
# child, a child without local location, children with inherit_scale FIX_SHEAR / ALIGNED /
# AVERAGE / NONE / NONE_LEGACY) posed in every rotation mode with non-uniform scale;
# PoseBone.matrix / head / tail / length / matrix_channel / matrix_basis, the matrix and
# matrix_basis setters, pose_position='REST'. Blender reads pose matrices as of the last
# depsgraph evaluation (D-04), so the script updates the view layer before reading.
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
bpy.data.objects.new('C8', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['C8'])
bpy.ops.object.armature_add(location=(0.5, -1, 0.25), rotation=(0.2, 0, 0.4))
bpy.ops.object.mode_set(mode='EDIT')
bpy.data.armatures['Armature'].edit_bones['Bone'].tail = (0.2, 0.1, 1.2)
bpy.data.armatures['Armature'].edit_bones['Bone'].roll = 0.3
bpy.data.armatures['Armature'].edit_bones.new('B1')
bpy.data.armatures['Armature'].edit_bones['B1'].head = (0.2, 0.1, 1.2)
bpy.data.armatures['Armature'].edit_bones['B1'].tail = (0.9, 0.4, 1.6)
bpy.data.armatures['Armature'].edit_bones['B1'].parent = bpy.data.armatures['Armature'].edit_bones['Bone']
bpy.data.armatures['Armature'].edit_bones['B1'].use_connect = True
bpy.data.armatures['Armature'].edit_bones.new('B2')
bpy.data.armatures['Armature'].edit_bones['B2'].head = (1.0, 0.5, 1.7)
bpy.data.armatures['Armature'].edit_bones['B2'].tail = (1.2, 1.4, 1.7)
bpy.data.armatures['Armature'].edit_bones['B2'].roll = -0.6
bpy.data.armatures['Armature'].edit_bones['B2'].parent = bpy.data.armatures['Armature'].edit_bones['B1']
bpy.data.armatures['Armature'].edit_bones['B2'].use_inherit_rotation = False
bpy.data.armatures['Armature'].edit_bones.new('B3')
bpy.data.armatures['Armature'].edit_bones['B3'].head = (-0.5, 0, 1)
bpy.data.armatures['Armature'].edit_bones['B3'].tail = (-0.6, 0.3, 2)
bpy.data.armatures['Armature'].edit_bones['B3'].parent = bpy.data.armatures['Armature'].edit_bones['Bone']
bpy.data.armatures['Armature'].edit_bones['B3'].use_local_location = False
bpy.data.armatures['Armature'].edit_bones['B3'].inherit_scale = 'FIX_SHEAR'
bpy.data.armatures['Armature'].edit_bones.new('B4')
bpy.data.armatures['Armature'].edit_bones['B4'].head = (0.9, 0.4, 1.6)
bpy.data.armatures['Armature'].edit_bones['B4'].tail = (0.9, -0.6, 2.0)
bpy.data.armatures['Armature'].edit_bones['B4'].parent = bpy.data.armatures['Armature'].edit_bones['B1']
bpy.data.armatures['Armature'].edit_bones['B4'].inherit_scale = 'ALIGNED'
bpy.data.armatures['Armature'].edit_bones.new('B5')
bpy.data.armatures['Armature'].edit_bones['B5'].head = (0.9, 0.4, 1.6)
bpy.data.armatures['Armature'].edit_bones['B5'].tail = (1.9, 0.4, 2.2)
bpy.data.armatures['Armature'].edit_bones['B5'].parent = bpy.data.armatures['Armature'].edit_bones['B1']
bpy.data.armatures['Armature'].edit_bones['B5'].inherit_scale = 'AVERAGE'
bpy.data.armatures['Armature'].edit_bones.new('B6')
bpy.data.armatures['Armature'].edit_bones['B6'].head = (0.9, 0.4, 1.6)
bpy.data.armatures['Armature'].edit_bones['B6'].tail = (0.4, 0.8, 1.1)
bpy.data.armatures['Armature'].edit_bones['B6'].parent = bpy.data.armatures['Armature'].edit_bones['B1']
bpy.data.armatures['Armature'].edit_bones['B6'].inherit_scale = 'NONE'
bpy.data.armatures['Armature'].edit_bones.new('B7')
bpy.data.armatures['Armature'].edit_bones['B7'].head = (0.9, 0.4, 1.6)
bpy.data.armatures['Armature'].edit_bones['B7'].tail = (0.9, 0.4, 0.6)
bpy.data.armatures['Armature'].edit_bones['B7'].parent = bpy.data.armatures['Armature'].edit_bones['B1']
bpy.data.armatures['Armature'].edit_bones['B7'].inherit_scale = 'NONE_LEGACY'
bpy.data.armatures['Armature'].edit_bones['B7'].use_inherit_rotation = False
bpy.ops.object.mode_set(mode='POSE')
bpy.data.objects['Armature'].pose.bones['Bone'].rotation_quaternion = (0.9, 0.2, -0.3, 0.1)
bpy.data.objects['Armature'].pose.bones['Bone'].scale = (1.5, 0.8, 1.2)
bpy.data.objects['Armature'].pose.bones['Bone'].location = (0.1, 0.2, -0.3)
bpy.data.objects['Armature'].pose.bones['B1'].rotation_mode = 'ZXY'
bpy.data.objects['Armature'].pose.bones['B1'].rotation_euler = (0.4, -0.2, 0.7)
bpy.data.objects['Armature'].pose.bones['B1'].location = (5, 5, 5)
bpy.data.objects['Armature'].pose.bones['B1'].scale = (0.7, 1.3, 1.1)
bpy.data.objects['Armature'].pose.bones['B2'].rotation_mode = 'AXIS_ANGLE'
bpy.data.objects['Armature'].pose.bones['B2'].rotation_axis_angle = (0.8, 0.3, 1, -0.2)
bpy.data.objects['Armature'].pose.bones['B2'].location = (0.3, 0, 0.1)
bpy.data.objects['Armature'].pose.bones['B3'].rotation_mode = 'YXZ'
bpy.data.objects['Armature'].pose.bones['B3'].rotation_euler = (-0.5, 0.25, 0.1)
bpy.data.objects['Armature'].pose.bones['B3'].location = (0.2, -0.4, 0.3)
bpy.data.objects['Armature'].pose.bones['B4'].rotation_quaternion = (0.8, 0.0, 0.5, 0.2)
bpy.data.objects['Armature'].pose.bones['B4'].scale = (2, 1, 1)
bpy.data.objects['Armature'].pose.bones['B6'].location = (0.1, 0.1, 0.1)
bpy.context.view_layer.update()
bpy.data.objects['C0'].matrix_world = bpy.data.objects['Armature'].pose.bones['Bone'].matrix
bpy.data.objects['C1'].matrix_world = bpy.data.objects['Armature'].pose.bones['B1'].matrix
bpy.data.objects['C2'].matrix_world = bpy.data.objects['Armature'].pose.bones['B2'].matrix
bpy.data.objects['C3'].matrix_world = bpy.data.objects['Armature'].pose.bones['B3'].matrix
bpy.data.objects['C4'].matrix_world = bpy.data.objects['Armature'].pose.bones['B4'].matrix
bpy.data.objects['C5'].matrix_world = bpy.data.objects['Armature'].pose.bones['B5'].matrix
bpy.data.objects['C6'].matrix_world = bpy.data.objects['Armature'].pose.bones['B6'].matrix
bpy.data.objects['C7'].matrix_world = bpy.data.objects['Armature'].pose.bones['B7'].matrix
bpy.data.objects['C8'].location = bpy.data.objects['Armature'].pose.bones['B2'].tail
bpy.data.objects['C8'].rotation_euler = bpy.data.objects['Armature'].pose.bones['B4'].head
bpy.data.objects['C8'].scale.x = bpy.data.objects['Armature'].pose.bones['B4'].length
bpy.data.objects['C8'].scale.y = bpy.data.objects['Armature'].pose.bones['B1'].length
bpy.data.objects['C8'].scale.z = bpy.data.objects['Armature'].pose.bones['Bone'].length
bpy.data.objects['Armature'].pose.bones['B1'].parent.name
bpy.data.objects['Armature'].pose.bones['Bone'].child.name
bpy.data.objects['Armature'].pose.bones['B1'].bone.name
