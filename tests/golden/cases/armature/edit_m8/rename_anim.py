# WP 8.3.10, bone renames carry the animation (ED_armature_bone_rename →
# BKE_animdata_fix_paths_rename_all(arm, "bones", old, new)): armature.flip_names with
# do_strip_numbers swaps the keyed pose-bone F-curves of an .L.001/.R pair (and their groups),
# armature.autoside_names(XAXIS) moves a keyed 'Bone' to 'Bone.L', the EditBone.name and
# Bone.name setters rename pose.bones["…"] paths too; an F-curve of a bone that is not renamed
# keeps its path.
# golden: startup=factory tier=T1 tol=1e-5 compare=anim:ArmatureAction,objects
import bpy
bpy.ops.object.armature_add()
bpy.ops.object.mode_set(mode='EDIT')
bpy.data.armatures['Armature'].edit_bones['Bone'].tail = (0.5, 0, 1)
bpy.data.armatures['Armature'].edit_bones.new('arm.L.001')
bpy.data.armatures['Armature'].edit_bones['arm.L.001'].head = (0.5, 0, 1)
bpy.data.armatures['Armature'].edit_bones['arm.L.001'].tail = (1, 0, 1)
bpy.data.armatures['Armature'].edit_bones.new('arm.R')
bpy.data.armatures['Armature'].edit_bones['arm.R'].head = (-0.5, 0, 1)
bpy.data.armatures['Armature'].edit_bones['arm.R'].tail = (-1, 0, 1)
bpy.data.armatures['Armature'].edit_bones.new('spine')
bpy.data.armatures['Armature'].edit_bones['spine'].head = (0, 0, 1)
bpy.data.armatures['Armature'].edit_bones['spine'].tail = (0, 0, 2)
bpy.ops.object.mode_set(mode='POSE')
bpy.data.objects['Armature'].pose.bones['arm.L.001'].keyframe_insert('location')
bpy.data.objects['Armature'].pose.bones['arm.R'].keyframe_insert('scale')
bpy.data.objects['Armature'].pose.bones['Bone'].keyframe_insert('location')
bpy.data.objects['Armature'].pose.bones['spine'].keyframe_insert('rotation_quaternion')
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.armature.select_all(action='DESELECT')
bpy.data.armatures['Armature'].edit_bones['arm.L.001'].select = True
bpy.data.armatures['Armature'].edit_bones['arm.L.001'].select_head = True
bpy.data.armatures['Armature'].edit_bones['arm.L.001'].select_tail = True
bpy.data.armatures['Armature'].edit_bones['arm.R'].select = True
bpy.data.armatures['Armature'].edit_bones['arm.R'].select_head = True
bpy.data.armatures['Armature'].edit_bones['arm.R'].select_tail = True
bpy.ops.armature.flip_names(do_strip_numbers=True)
bpy.data.armatures['Armature'].edit_bones['arm.R']
bpy.data.armatures['Armature'].edit_bones['arm.L']
bpy.data.objects['Armature'].animation_data.action.layers[0].strips[0].channelbags[0].fcurves.find('pose.bones["arm.R"].location', index=0).data_path
bpy.data.objects['Armature'].animation_data.action.layers[0].strips[0].channelbags[0].fcurves.find('pose.bones["arm.L"].scale', index=2).data_path
bpy.data.objects['Armature'].animation_data.action.layers[0].strips[0].channelbags[0].groups['arm.R']
bpy.data.objects['Armature'].animation_data.action.layers[0].strips[0].channelbags[0].groups['arm.L']
bpy.ops.armature.select_all(action='DESELECT')
bpy.data.armatures['Armature'].edit_bones['Bone'].select = True
bpy.data.armatures['Armature'].edit_bones['Bone'].select_head = True
bpy.data.armatures['Armature'].edit_bones['Bone'].select_tail = True
bpy.ops.armature.autoside_names(type='XAXIS')
bpy.data.armatures['Armature'].edit_bones['Bone.L']
bpy.data.objects['Armature'].animation_data.action.layers[0].strips[0].channelbags[0].fcurves.find('pose.bones["Bone.L"].location', index=1).data_path
bpy.data.armatures['Armature'].edit_bones['spine'].name = 'torso'
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Armature'].animation_data.action.layers[0].strips[0].channelbags[0].fcurves.find('pose.bones["torso"].rotation_quaternion', index=3).data_path
bpy.data.armatures['Armature'].bones['torso'].name = 'chest'
bpy.data.objects['Armature'].pose.bones['chest'].name = 'neck'
bpy.data.objects['Armature'].animation_data.action.layers[0].strips[0].channelbags[0].groups['neck']
