# WP 6.5 fix: Object.parent_bone_head_tail_factor places a bone child along its parent bone
# (ob_parbone: pose_mat[1] * length * factor); the parent inverse of parent_set(type='BONE')
# uses the factor the child has, and a later change moves the child.
# golden: startup=factory tier=T1 tol=1e-5 compare=objects
import bpy
bpy.ops.object.armature_add(location=(0, 0, -1))
bpy.data.objects['Armature'].pose.bones['Bone'].rotation_quaternion = (0.9, 0.2, 0.3, 0.0)
bpy.data.objects['Camera'].parent_bone_head_tail_factor
bpy.data.objects['Camera'].parent_bone_head_tail_factor = 0.25
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Cube'].select_set(True)
bpy.data.objects['Camera'].select_set(True)
bpy.data.objects['Armature'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Armature']
bpy.ops.object.parent_set(type='BONE')
bpy.data.objects['Cube'].parent_bone_head_tail_factor = 0.0
bpy.data.objects['Light'].parent_bone_head_tail_factor = 1.5
bpy.data.objects['Light'].parent_bone_head_tail_factor
bpy.data.objects['Armature'].pose.bones['Bone'].rotation_quaternion = (0.7, 0.2, 0.3, 0.5)
