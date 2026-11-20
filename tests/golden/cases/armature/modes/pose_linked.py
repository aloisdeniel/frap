# WP 6.2, mode_set(mode='POSE') with two selected armatures sharing one Armature
# (duplicate_move_linked): both objects enter Pose Mode (the pose is per object), and the
# selected and visible pose bones list both objects' channels; leaving takes both out.
# golden: startup=factory compare=objects,context tier=T0 tol=1e-5
import bpy
bpy.data.objects.new('C0', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['C0'])
bpy.data.objects.new('C1', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['C1'])
bpy.ops.object.armature_add()
bpy.ops.object.duplicate_move_linked(TRANSFORM_OT_translate={'value': (2, 0, 0)})
bpy.data.objects['Armature'].select_set(True)
bpy.ops.object.mode_set(mode='POSE')
bpy.data.objects['Armature'].mode
bpy.data.objects['Armature.001'].mode
bpy.data.objects['C0'].location.x = bpy.context.objects_in_mode[1].location.x
bpy.data.objects['C0'].location.y = bpy.context.objects_in_mode[0].location.x
bpy.context.selected_pose_bones[1].name
bpy.context.visible_pose_bones[1].name
bpy.context.selected_pose_bones[2].name
bpy.ops.pose.select_all(action='DESELECT')
bpy.context.selected_pose_bones[0].name
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['C1'].location.y = bpy.data.objects['Armature.001'].pose.bones['Bone'].select
bpy.context.objects_in_mode[0].name
