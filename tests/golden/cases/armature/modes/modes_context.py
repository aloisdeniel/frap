# WP 6.2, object.mode_set EDIT/POSE for armatures (MODE_HANDLERS), object.posemode_toggle,
# object.editmode_toggle: context.mode, the pose selection carried into Edit Mode and back
# (pose bones keep the selection since Blender 5.0), multi-object Pose Mode, and the bone
# members of bpy.context (active_bone, active_pose_bone, pose_object, selected_bones,
# selected_editable_bones, editable_bones, visible_bones, selected_pose_bones,
# selected_pose_bones_from_active_object, visible_pose_bones) in each mode.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.data.objects.new('C0', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['C0'])
bpy.data.objects.new('C1', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['C1'])
bpy.ops.object.armature_add()
bpy.context.active_bone.name
bpy.context.active_pose_bone
bpy.context.pose_object
bpy.context.selected_pose_bones
bpy.context.selected_bones
bpy.ops.object.posemode_toggle()
bpy.context.mode
bpy.context.pose_object.name
bpy.context.active_pose_bone.name
bpy.context.active_bone.name
bpy.context.selected_pose_bones[0]
bpy.context.visible_pose_bones[0].name
bpy.ops.pose.select_all(action='SELECT')
bpy.context.selected_pose_bones[0].name
bpy.context.selected_pose_bones_from_active_object[0].name
bpy.context.selected_bones
bpy.ops.object.mode_set(mode='EDIT')
bpy.context.mode
bpy.context.edit_object.name
bpy.data.objects['C0'].location.x = bpy.data.armatures['Armature'].edit_bones['Bone'].select
bpy.data.objects['C0'].location.y = bpy.data.armatures['Armature'].edit_bones['Bone'].select_head
bpy.data.objects['C0'].location.z = bpy.data.armatures['Armature'].edit_bones['Bone'].select_tail
bpy.context.selected_bones[0].name
bpy.context.selected_editable_bones[0].name
bpy.context.editable_bones[0].name
bpy.context.visible_bones[0].name
bpy.context.active_bone.name
bpy.context.selected_pose_bones
bpy.ops.armature.extrude_move(TRANSFORM_OT_translate={'value': (0, 0, 1)})
bpy.context.selected_bones[0].name
bpy.context.selected_bones[1]
bpy.data.armatures['Armature'].edit_bones['Bone'].hide = True
bpy.context.visible_bones[1]
bpy.data.armatures['Armature'].edit_bones['Bone'].hide = False
bpy.data.armatures['Armature'].edit_bones['Bone.001'].lock = True
bpy.context.editable_bones
bpy.ops.object.editmode_toggle()
bpy.context.mode
bpy.data.objects['Armature'].pose.bones['Bone.001'].name
bpy.data.objects['C0'].rotation_euler.x = bpy.data.objects['Armature'].pose.bones['Bone.001'].select
bpy.data.objects['C0'].rotation_euler.y = bpy.data.objects['Armature'].pose.bones['Bone'].select
bpy.ops.object.mode_set(mode='POSE')
bpy.context.selected_pose_bones[0].name
bpy.context.active_pose_bone.name
bpy.ops.object.mode_set(mode='EDIT', toggle=True)
bpy.context.mode
bpy.ops.object.mode_set(mode='POSE')
bpy.ops.object.mode_set(mode='POSE', toggle=True)
bpy.context.mode
bpy.ops.object.armature_add(location=(2, 0, 0))
bpy.data.objects['Armature'].select_set(True)
bpy.ops.object.posemode_toggle()
bpy.context.visible_pose_bones[1].name
bpy.ops.pose.select_all(action='DESELECT')
bpy.ops.object.posemode_toggle()
bpy.context.mode
bpy.data.objects['Cube'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.object.posemode_toggle()
bpy.context.mode
bpy.ops.object.mode_set(mode='POSE')
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.object.posemode_toggle()
bpy.context.mode
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.view_layer.objects.active = bpy.data.objects['Armature.001']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.object.posemode_toggle()
bpy.context.mode
bpy.data.objects['Armature.001'].mode
bpy.ops.pose.transforms_clear()
bpy.ops.armature.select_all()
