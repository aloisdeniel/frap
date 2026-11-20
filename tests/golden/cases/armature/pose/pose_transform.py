# WP 6.2, transform.translate/rotate/resize in Pose Mode (TRANSFORM_MODE_HANDLERS, pose):
# world values turned into bone space through the parent transform and the object matrix;
# children of a transformed bone stay out; LOCAL/NORMAL/GIMBAL orientations, constraint axes,
# median/cursor/individual-origins/active pivots; location, rotation (quaternion, Euler,
# axis-angle, 4D) and scale locks; a bone without local location; nothing selected cancels.
# (Locked quaternions are not unit length: their components are captured directly.)
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
bpy.ops.object.armature_add(location=(1, 0, 0), rotation=(0, 0, 0.5))
bpy.data.objects['Armature'].scale = (1, 2, 1)
bpy.ops.object.mode_set(mode='EDIT')
bpy.data.armatures['Armature'].edit_bones['Bone'].roll = 0.4
bpy.data.armatures['Armature'].edit_bones.new('B1')
bpy.data.armatures['Armature'].edit_bones['B1'].head = (0, 0, 1)
bpy.data.armatures['Armature'].edit_bones['B1'].tail = (1, 0, 1.5)
bpy.data.armatures['Armature'].edit_bones['B1'].parent = bpy.data.armatures['Armature'].edit_bones['Bone']
bpy.data.armatures['Armature'].edit_bones.new('B2')
bpy.data.armatures['Armature'].edit_bones['B2'].head = (-1, 0, 0)
bpy.data.armatures['Armature'].edit_bones['B2'].tail = (-1, 1, 0)
bpy.data.armatures['Armature'].edit_bones['B2'].use_local_location = False
bpy.data.armatures['Armature'].edit_bones.new('B3')
bpy.data.armatures['Armature'].edit_bones['B3'].head = (2, 0, 0)
bpy.data.armatures['Armature'].edit_bones['B3'].tail = (2, 0.5, 1)
bpy.data.armatures['Armature'].edit_bones['B3'].parent = bpy.data.armatures['Armature'].edit_bones['B1']
bpy.data.armatures['Armature'].edit_bones['B3'].use_local_location = False
bpy.ops.object.mode_set(mode='POSE')
bpy.ops.transform.translate(value=(1, 0, 0))
bpy.data.objects['Armature'].pose.bones['B1'].select = True
bpy.ops.transform.translate(value=(0.5, 0.25, -1))
bpy.data.objects['C0'].location = bpy.data.objects['Armature'].pose.bones['B1'].location
bpy.data.objects['Armature'].pose.bones['Bone'].select = True
bpy.ops.transform.rotate(value=0.6, orient_axis='Y')
bpy.data.objects['C0'].rotation_euler = bpy.data.objects['Armature'].pose.bones['Bone'].rotation_quaternion.to_euler()
bpy.data.objects['C0'].scale = bpy.data.objects['Armature'].pose.bones['B1'].rotation_quaternion.to_euler()
bpy.ops.pose.select_all(action='DESELECT')
bpy.data.objects['Armature'].pose.bones['B1'].select = True
bpy.data.objects['Armature'].pose.bones['B1'].rotation_mode = 'XZY'
bpy.ops.transform.rotate(value=-0.3, orient_axis='X', orient_type='LOCAL', constraint_axis=(True, False, False))
bpy.data.objects['C1'].rotation_euler = bpy.data.objects['Armature'].pose.bones['B1'].rotation_euler
bpy.ops.transform.resize(value=(1.5, 1, 0.5), orient_type='LOCAL')
bpy.data.objects['C1'].scale = bpy.data.objects['Armature'].pose.bones['B1'].scale
bpy.data.objects['C1'].location = bpy.data.objects['Armature'].pose.bones['B1'].location
bpy.ops.transform.rotate(value=0.25, orient_axis='Z', orient_type='GIMBAL', constraint_axis=(False, False, True))
bpy.data.objects['C2'].rotation_euler = bpy.data.objects['Armature'].pose.bones['B1'].rotation_euler
bpy.ops.transform.translate(value=(0, 0, 0.5), orient_type='NORMAL', constraint_axis=(False, False, True))
bpy.data.objects['C2'].location = bpy.data.objects['Armature'].pose.bones['B1'].location
bpy.ops.pose.select_all(action='SELECT')
bpy.context.scene.tool_settings.transform_pivot_point = 'INDIVIDUAL_ORIGINS'
bpy.ops.transform.rotate(value=0.4, orient_axis='X')
bpy.data.objects['C3'].rotation_euler = bpy.data.objects['Armature'].pose.bones['B2'].rotation_quaternion.to_euler()
bpy.data.objects['C3'].location = bpy.data.objects['Armature'].pose.bones['B2'].location
bpy.data.objects['C3'].scale = bpy.data.objects['Armature'].pose.bones['B3'].location
bpy.context.scene.tool_settings.transform_pivot_point = 'CURSOR'
bpy.context.scene.cursor.location = (0, 3, 1)
bpy.ops.transform.rotate(value=0.3, orient_axis='Z')
bpy.ops.transform.resize(value=(1.2, 1.2, 1.2))
bpy.data.objects['C4'].location = bpy.data.objects['Armature'].pose.bones['B2'].location
bpy.data.objects['C4'].scale = bpy.data.objects['Armature'].pose.bones['B2'].scale
bpy.data.objects['C4'].rotation_euler = bpy.data.objects['Armature'].pose.bones['Bone'].location
bpy.context.scene.tool_settings.transform_pivot_point = 'ACTIVE_ELEMENT'
bpy.data.armatures['Armature'].bones.active = bpy.data.armatures['Armature'].bones['B2']
bpy.ops.transform.rotate(value=0.2, orient_axis='Y')
bpy.data.objects['C5'].location = bpy.data.objects['Armature'].pose.bones['Bone'].location
bpy.data.objects['C5'].rotation_euler = bpy.data.objects['Armature'].pose.bones['Bone'].rotation_quaternion.to_euler()
bpy.context.scene.tool_settings.transform_pivot_point = 'MEDIAN_POINT'
bpy.data.objects['Armature'].pose.bones['Bone'].lock_location = (True, False, True)
bpy.data.objects['Armature'].pose.bones['Bone'].lock_rotation = (False, True, False)
bpy.data.objects['Armature'].pose.bones['Bone'].lock_scale = (False, False, True)
bpy.data.objects['Armature'].pose.bones['B2'].lock_rotation_w = True
bpy.data.objects['Armature'].pose.bones['B2'].lock_rotation = (True, False, False)
bpy.ops.transform.translate(value=(0.3, 0.3, 0.3))
bpy.ops.transform.rotate(value=0.5, orient_axis='X')
bpy.ops.transform.resize(value=(0.5, 0.5, 0.5))
bpy.data.objects['C6'].location = bpy.data.objects['Armature'].pose.bones['Bone'].location
bpy.data.objects['C6'].rotation_euler = bpy.data.objects['Armature'].pose.bones['Bone'].rotation_quaternion[1:4]
bpy.data.objects['C6'].scale = bpy.data.objects['Armature'].pose.bones['Bone'].scale
bpy.data.objects['C7'].rotation_euler.x = bpy.data.objects['Armature'].pose.bones['Bone'].rotation_quaternion[0]
bpy.data.objects['C7'].location = bpy.data.objects['Armature'].pose.bones['B2'].rotation_quaternion[0:3]
bpy.data.objects['Armature'].pose.bones['B2'].lock_rotations_4d = False
bpy.ops.transform.rotate(value=0.5, orient_axis='Z')
bpy.data.objects['C7'].scale = bpy.data.objects['Armature'].pose.bones['B2'].rotation_quaternion[1:4]
bpy.ops.pose.select_all(action='DESELECT')
bpy.ops.transform.translate(value=(1, 0, 0))
