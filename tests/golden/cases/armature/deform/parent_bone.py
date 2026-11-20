# WP 6.5: parenting to bones. object.parent_set BONE / BONE_RELATIVE parent to the active bone
# (parent_type BONE, parent_bone, use_relative_parent cleared / set) with
# matrix_parent_inverse inverting the posed bone's world matrix at its tail (BONE) or the bone's
# deform matrix (BONE_RELATIVE), with and without keep_transform; children follow later pose
# changes; "No active bone" for an armature whose active bone is in a hidden collection, without
# an active bone, and for a parent that is not an armature; parent_type = 'BONE' and parent_bone set through RNA,
# a missing bone (the parent's world matrix), and a mesh child with a modifier stack.
# golden: startup=factory tier=T1 tol=1e-5 compare=objects,evaluated:Cube
import bpy
bpy.ops.object.armature_add(location=(0.5, -0.5, 0.2), rotation=(0.0, 0.3, 0.2))
bpy.ops.object.mode_set(mode='EDIT')
bpy.data.armatures['Armature'].edit_bones['Bone'].tail = (0, 0.2, 1.2)
bpy.data.armatures['Armature'].edit_bones.new('Tip')
bpy.data.armatures['Armature'].edit_bones['Tip'].head = (0, 0.2, 1.2)
bpy.data.armatures['Armature'].edit_bones['Tip'].tail = (0.6, 0.2, 1.8)
bpy.data.armatures['Armature'].edit_bones['Tip'].roll = 0.4
bpy.data.armatures['Armature'].edit_bones['Tip'].parent = bpy.data.armatures['Armature'].edit_bones['Bone']
bpy.data.armatures['Armature'].edit_bones['Tip'].use_connect = True
bpy.ops.object.mode_set(mode='POSE')
bpy.data.objects['Armature'].pose.bones['Bone'].rotation_quaternion = (0.9, 0.1, 0.3, -0.2)
bpy.data.objects['Armature'].pose.bones['Tip'].rotation_quaternion = (0.8, 0.4, 0.0, 0.3)
bpy.data.objects['Armature'].pose.bones['Tip'].scale = (1.1, 1.5, 0.9)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects.new('E1', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['E1'])
bpy.data.objects['E1'].location = (1, 2, 3)
bpy.data.objects['E1'].rotation_euler = (0.3, 0.2, 0.1)
bpy.data.objects.new('E2', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['E2'])
bpy.data.objects['E2'].location = (-1, 0.5, 2)
bpy.data.objects.new('E3', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['E3'])
bpy.data.objects['E3'].location = (0.2, 0.2, 0.2)
bpy.data.objects['E3'].parent = bpy.data.objects['Camera']
bpy.data.objects.new('E4', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['E4'])
bpy.data.objects['E4'].location = (0.1, 0.2, 0.3)
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['E1'].select_set(True)
bpy.data.objects['Cube'].select_set(True)
bpy.data.objects['Armature'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Armature']
bpy.data.armatures['Armature'].bones.active = bpy.data.armatures['Armature'].bones['Tip']
bpy.ops.object.parent_set(type='BONE')
bpy.data.objects['E1'].parent_type
bpy.data.objects['E1'].parent_bone
#@blender assert bpy.data.objects['E1'].parent_bone == 'Tip' and bpy.data.objects['Cube'].parent_type == 'BONE'
bpy.data.armatures['Armature'].bones['Tip'].use_relative_parent
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['E2'].select_set(True)
bpy.data.objects['E3'].select_set(True)
bpy.data.objects['Armature'].select_set(True)
bpy.data.armatures['Armature'].bones.active = bpy.data.armatures['Armature'].bones['Bone']
bpy.ops.object.parent_set(type='BONE_RELATIVE', keep_transform=True)
bpy.data.armatures['Armature'].bones['Bone'].use_relative_parent
#@blender assert bpy.data.armatures['Armature'].bones['Bone'].use_relative_parent
bpy.data.objects['Cube'].modifiers.new('Bend', 'SIMPLE_DEFORM')
bpy.data.objects['Armature'].pose.bones['Bone'].rotation_quaternion = (0.7, -0.3, 0.2, 0.5)
bpy.data.objects['Armature'].pose.bones['Bone'].location = (0.2, 0.0, 0.1)
bpy.data.objects['Armature'].pose.bones['Tip'].rotation_quaternion = (1.0, 0.0, 0.0, 0.0)
bpy.data.objects['E4'].parent = bpy.data.objects['Armature']
bpy.data.objects['E4'].parent_type = 'BONE'
bpy.data.objects['E4'].parent_bone = 'Tip'
bpy.data.objects.new('E5', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['E5'])
bpy.data.objects['E5'].parent = bpy.data.objects['Armature']
bpy.data.objects['E5'].parent_type = 'BONE'
bpy.data.objects['E5'].parent_bone = 'Nope'
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['E5'].select_set(True)
bpy.data.objects['Camera'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Camera']
bpy.ops.object.parent_set(type='BONE')
bpy.context.view_layer.objects.active = bpy.data.objects['Armature']
bpy.data.armatures['Armature'].collections_all['Bones'].is_visible = False
bpy.ops.object.parent_set(type='BONE_RELATIVE')
bpy.data.armatures['Armature'].collections_all['Bones'].is_visible = True
bpy.data.armatures['Armature'].bones.active = None
bpy.ops.object.parent_set(type='BONE')
bpy.data.objects['E5'].parent_type
