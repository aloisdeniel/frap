# WP 6.4: keying pose bones. PoseBone.keyframe_insert keys `pose.bones["Bone"].…` in the
# armature object's action, grouped by bone name; anim.keyframe_insert in Pose Mode keys the
# selected bones (rotation by each bone's rotation mode); keyframe_delete_v3d in Pose Mode only
# touches the selected bones' channels; frame_set applies the pose animation.
# golden: startup=factory tier=T1 tol=1e-5 compare=anim:ArmatureAction,objects
import bpy
bpy.ops.object.armature_add(location=(0.0, 0.0, 0.0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.armature.extrude_move(TRANSFORM_OT_translate={"value": (0.0, 0.0, 1.0)})
bpy.ops.object.mode_set(mode='POSE')
bpy.data.objects['Armature'].pose.bones['Bone'].keyframe_insert('rotation_quaternion', frame=1)
bpy.data.objects['Armature'].pose.bones['Bone'].rotation_quaternion = (0.7071068, 0.7071068, 0.0, 0.0)
bpy.data.objects['Armature'].pose.bones['Bone'].keyframe_insert('rotation_quaternion', frame=9)
bpy.data.objects['Armature'].pose.bones['Bone'].keyframe_insert('location', index=1, frame=9)
bpy.data.actions['ArmatureAction'].layers[0].strips[0].channelbags[0].groups['Bone']
bpy.data.actions['ArmatureAction'].slots['OBArmature']
bpy.data.objects['Armature'].pose.bones['Bone'].keyframe_insert('pose.bones["Bone"].location')
bpy.data.objects['Armature'].pose.bones['Bone.001'].rotation_mode = 'XYZ'
bpy.ops.pose.select_all(action='SELECT')
bpy.context.scene.frame_current = 5
bpy.ops.anim.keyframe_insert()
bpy.data.actions['ArmatureAction'].layers[0].strips[0].channelbags[0].groups['Bone.001']
bpy.data.actions['ArmatureAction'].layers[0].strips[0].channelbags[0].fcurves.find('pose.bones["Bone.001"].rotation_euler', index=2)
bpy.ops.pose.select_all(action='DESELECT')
bpy.ops.anim.keyframe_delete_v3d()
bpy.context.scene.frame_set(9)
bpy.data.objects['Cube'].rotation_quaternion = bpy.data.objects['Armature'].pose.bones['Bone'].rotation_quaternion
bpy.context.scene.frame_set(3)
bpy.data.objects['Cube'].location.x = bpy.data.objects['Armature'].pose.bones['Bone'].rotation_quaternion.x
