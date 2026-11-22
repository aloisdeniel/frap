# WP 8.3.7: the bone subsets of `group_select_mode` and the bone order of `vertex_group_sort`.
# A mesh deformed by an ARMATURE modifier: `BONE_DEFORM` is the groups named after deforming
# bones (`BKE_object_defgroup_validmap_get`; the non-deforming Root is not one), offered only with
# an armature; with the armature in Pose Mode `BONE_SELECT` is the groups of its selected pose
# bones. `vertex_group_sort(sort_type='BONE_HIERARCHY')` puts the bones' groups first in
# depth-first bone order. Group order is read into object transforms; weights through DISPLACE
# modifiers (Spine along X, Arm.L along Y, Other along Z).
# golden: startup=factory tier=T1 compare=objects,evaluated:Cube
import bpy
bpy.ops.object.armature_add()
bpy.ops.object.mode_set(mode='EDIT')
bpy.data.armatures['Armature'].edit_bones['Bone'].name = 'Root'
bpy.data.armatures['Armature'].edit_bones['Root'].use_deform = False
bpy.data.armatures['Armature'].edit_bones.new('Spine')
bpy.data.armatures['Armature'].edit_bones['Spine'].head = (0, 0, 1)
bpy.data.armatures['Armature'].edit_bones['Spine'].tail = (0, 0, 2)
bpy.data.armatures['Armature'].edit_bones['Spine'].parent = bpy.data.armatures['Armature'].edit_bones['Root']
bpy.data.armatures['Armature'].edit_bones.new('Arm.L')
bpy.data.armatures['Armature'].edit_bones['Arm.L'].head = (0, 0, 2)
bpy.data.armatures['Armature'].edit_bones['Arm.L'].tail = (1, 0, 2)
bpy.data.armatures['Armature'].edit_bones['Arm.L'].parent = bpy.data.armatures['Armature'].edit_bones['Spine']
bpy.data.armatures['Armature'].edit_bones.new('Arm.R')
bpy.data.armatures['Armature'].edit_bones['Arm.R'].head = (0, 0, 2)
bpy.data.armatures['Armature'].edit_bones['Arm.R'].tail = (-1, 0, 2)
bpy.data.armatures['Armature'].edit_bones['Arm.R'].parent = bpy.data.armatures['Armature'].edit_bones['Spine']
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Armature'].location = (5, 0, 0)
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Cube'].select_set(True)
bpy.data.objects['Cube'].vertex_groups.new(name='Other')
bpy.data.objects['Cube'].vertex_groups.new(name='Arm.R')
bpy.data.objects['Cube'].vertex_groups.new(name='Root')
bpy.data.objects['Cube'].vertex_groups.new(name='Arm.L')
bpy.data.objects['Cube'].vertex_groups.new(name='Spine')
bpy.data.objects['Cube'].vertex_groups.new(name='zz')
bpy.data.objects['Cube'].vertex_groups['Other'].add([0, 1, 2, 3], 0.2, 'REPLACE')
bpy.data.objects['Cube'].vertex_groups['Arm.L'].add([0, 4, 5], 0.3, 'REPLACE')
bpy.data.objects['Cube'].vertex_groups['Spine'].add([1, 4, 6, 7], 0.6, 'REPLACE')
bpy.data.objects['Cube'].vertex_groups['Root'].add([2, 5, 6], 0.9, 'REPLACE')
bpy.ops.object.vertex_group_levels(group_select_mode='BONE_DEFORM', offset=0.1)
bpy.data.objects['Cube'].modifiers.new(name='Rig', type='ARMATURE')
bpy.data.objects['Cube'].modifiers['Rig'].object = bpy.data.objects['Armature']
bpy.ops.object.vertex_group_levels(group_select_mode='BONE_DEFORM', offset=0.1)
bpy.ops.object.vertex_group_invert(group_select_mode='BONE_DEFORM', auto_assign=False)
bpy.ops.object.vertex_group_quantize(group_select_mode='BONE_SELECT', steps=2)
bpy.data.objects['Cube'].modifiers['Rig'].show_viewport = False
bpy.ops.object.vertex_group_quantize(group_select_mode='BONE_DEFORM', steps=4)
bpy.data.objects['Cube'].modifiers['Rig'].show_viewport = True
bpy.ops.object.vertex_group_sort(sort_type='BONE_HIERARCHY')
bpy.data.objects['Camera'].location = (bpy.data.objects['Cube'].vertex_groups['Root'].index, bpy.data.objects['Cube'].vertex_groups['Spine'].index, bpy.data.objects['Cube'].vertex_groups['Arm.L'].index)
bpy.data.objects['Camera'].scale = (bpy.data.objects['Cube'].vertex_groups['Arm.R'].index, bpy.data.objects['Cube'].vertex_groups['Other'].index, bpy.data.objects['Cube'].vertex_groups['zz'].index)
bpy.data.objects['Camera'].rotation_euler = (bpy.data.objects['Cube'].vertex_groups.active_index, 0, 0)
bpy.context.view_layer.objects.active = bpy.data.objects['Armature']
bpy.ops.object.mode_set(mode='POSE')
bpy.data.objects['Armature'].pose.bones['Arm.L'].select = True
bpy.data.objects['Armature'].pose.bones['Spine'].select = False
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.object.vertex_group_levels(group_select_mode='BONE_SELECT', gain=2.0)
bpy.context.view_layer.objects.active = bpy.data.objects['Armature']
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.data.objects['Cube'].modifiers.new(name='A', type='DISPLACE')
bpy.data.objects['Cube'].modifiers['A'].vertex_group = 'Spine'
bpy.data.objects['Cube'].modifiers['A'].direction = 'X'
bpy.data.objects['Cube'].modifiers.new(name='B', type='DISPLACE')
bpy.data.objects['Cube'].modifiers['B'].vertex_group = 'Arm.L'
bpy.data.objects['Cube'].modifiers['B'].direction = 'Y'
bpy.data.objects['Cube'].modifiers.new(name='C', type='DISPLACE')
bpy.data.objects['Cube'].modifiers['C'].vertex_group = 'Other'
bpy.data.objects['Cube'].modifiers['C'].direction = 'Z'
