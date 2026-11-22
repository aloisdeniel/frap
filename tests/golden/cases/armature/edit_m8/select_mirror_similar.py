# WP 8.3.10, armature.select_mirror(only_active, extend) and armature.select_similar for every
# type: LENGTH and DIRECTION in world space (non-uniform object scale) with threshold,
# PREFIX / SUFFIX, BONE_COLLECTION, CHILDREN / CHILDREN_IMMEDIATE / SIBLINGS, SHAPE (pose-bone
# custom shapes), COLOR (every bone has the default colour); no active bone is an error.
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
bpy.data.objects.new('N0', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N0'])
bpy.ops.object.armature_add()
bpy.data.objects['Armature'].scale = (1, 2, 1)
bpy.ops.object.mode_set(mode='EDIT')
bpy.data.armatures['Armature'].edit_bones.new('arm.L')
bpy.data.armatures['Armature'].edit_bones['arm.L'].head = (0.2, 0, 1)
bpy.data.armatures['Armature'].edit_bones['arm.L'].tail = (1.2, 0, 1)
bpy.data.armatures['Armature'].edit_bones['arm.L'].parent = bpy.data.armatures['Armature'].edit_bones['Bone']
bpy.data.armatures['Armature'].edit_bones.new('arm.R')
bpy.data.armatures['Armature'].edit_bones['arm.R'].head = (-0.2, 0, 1)
bpy.data.armatures['Armature'].edit_bones['arm.R'].tail = (-1.2, 0, 1)
bpy.data.armatures['Armature'].edit_bones['arm.R'].parent = bpy.data.armatures['Armature'].edit_bones['Bone']
bpy.data.armatures['Armature'].edit_bones.new('L_hand')
bpy.data.armatures['Armature'].edit_bones['L_hand'].head = (1.2, 0, 1)
bpy.data.armatures['Armature'].edit_bones['L_hand'].tail = (1.2, 0.5, 1)
bpy.data.armatures['Armature'].edit_bones['L_hand'].parent = bpy.data.armatures['Armature'].edit_bones['arm.L']
bpy.data.armatures['Armature'].edit_bones['L_hand'].use_connect = True
bpy.data.armatures['Armature'].edit_bones.new('R_hand')
bpy.data.armatures['Armature'].edit_bones['R_hand'].head = (-1.2, 0, 1)
bpy.data.armatures['Armature'].edit_bones['R_hand'].tail = (-1.2, 0, 1.5)
bpy.data.armatures['Armature'].edit_bones['R_hand'].parent = bpy.data.armatures['Armature'].edit_bones['arm.R']
bpy.data.armatures['Armature'].edit_bones.new('leg.L')
bpy.data.armatures['Armature'].edit_bones['leg.L'].head = (0.3, 0, 0)
bpy.data.armatures['Armature'].edit_bones['leg.L'].tail = (0.3, 0, -1.05)
bpy.data.armatures['Armature'].edit_bones.new('spine')
bpy.data.armatures['Armature'].edit_bones['spine'].head = (0, 0, 1)
bpy.data.armatures['Armature'].edit_bones['spine'].tail = (0, 0, 2)
bpy.data.armatures['Armature'].edit_bones['spine'].parent = bpy.data.armatures['Armature'].edit_bones['Bone']
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Armature'].pose.bones['arm.L'].custom_shape = bpy.data.objects['Cube']
bpy.data.objects['Armature'].pose.bones['leg.L'].custom_shape = bpy.data.objects['Cube']
bpy.data.objects['Armature'].pose.bones['spine'].custom_shape = bpy.data.objects['Light']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.armature.select_all(action='DESELECT')
bpy.data.armatures['Armature'].edit_bones['arm.L'].select = True
bpy.data.armatures['Armature'].edit_bones['arm.L'].select_tail = True
bpy.data.armatures['Armature'].edit_bones['spine'].select = True
bpy.data.armatures['Armature'].edit_bones.active = bpy.data.armatures['Armature'].edit_bones['arm.L']
bpy.ops.armature.select_mirror()
bpy.data.objects['N0'].name = bpy.context.active_bone.name
bpy.data.objects['C0'].location.x = bpy.data.armatures['Armature'].edit_bones['arm.L'].select
bpy.data.objects['C0'].location.y = bpy.data.armatures['Armature'].edit_bones['arm.R'].select
bpy.data.objects['C0'].location.z = bpy.data.armatures['Armature'].edit_bones['arm.R'].select_tail
bpy.data.objects['C0'].rotation_euler.x = bpy.data.armatures['Armature'].edit_bones['spine'].select
bpy.data.objects['C0'].rotation_euler.y = bpy.data.armatures['Armature'].edit_bones['R_hand'].select_head
bpy.data.objects['C0'].rotation_euler.z = bpy.data.armatures['Armature'].edit_bones['L_hand'].select_head
bpy.ops.armature.select_mirror(extend=True)
bpy.data.objects['C0'].scale.x = bpy.data.armatures['Armature'].edit_bones['arm.L'].select
bpy.data.objects['C0'].scale.y = bpy.data.armatures['Armature'].edit_bones['arm.R'].select
bpy.ops.armature.select_all(action='DESELECT')
bpy.data.armatures['Armature'].edit_bones['L_hand'].select = True
bpy.data.armatures['Armature'].edit_bones['L_hand'].select_tail = True
bpy.data.armatures['Armature'].edit_bones['arm.L'].select = True
bpy.data.armatures['Armature'].edit_bones['arm.L'].select_tail = True
bpy.ops.armature.select_mirror(only_active=True, extend=True)
bpy.data.objects['C0'].scale.z = bpy.data.armatures['Armature'].edit_bones['R_hand'].select
bpy.data.objects['C1'].location.x = bpy.data.armatures['Armature'].edit_bones['arm.L'].select
bpy.data.objects['C1'].location.y = bpy.data.armatures['Armature'].edit_bones['L_hand'].select
bpy.data.objects['C1'].location.z = bpy.data.armatures['Armature'].edit_bones['arm.R'].select
bpy.ops.armature.select_all(action='DESELECT')
bpy.data.armatures['Armature'].edit_bones.active = bpy.data.armatures['Armature'].edit_bones['arm.L']
bpy.ops.armature.select_similar(type='LENGTH')
bpy.data.objects['C1'].rotation_euler.x = bpy.data.armatures['Armature'].edit_bones['arm.R'].select
bpy.data.objects['C1'].rotation_euler.y = bpy.data.armatures['Armature'].edit_bones['leg.L'].select
bpy.data.objects['C1'].rotation_euler.z = bpy.data.armatures['Armature'].edit_bones['spine'].select
bpy.data.objects['C1'].scale.x = bpy.data.armatures['Armature'].edit_bones['Bone'].select
bpy.ops.armature.select_all(action='DESELECT')
bpy.ops.armature.select_similar(type='LENGTH', threshold=0.04)
bpy.data.objects['C1'].scale.y = bpy.data.armatures['Armature'].edit_bones['leg.L'].select
bpy.data.objects['C1'].scale.z = bpy.data.armatures['Armature'].edit_bones['arm.R'].select
bpy.ops.armature.select_all(action='DESELECT')
bpy.ops.armature.select_similar(type='DIRECTION')
bpy.data.objects['C2'].location.x = bpy.data.armatures['Armature'].edit_bones['arm.R'].select
bpy.data.objects['C2'].location.y = bpy.data.armatures['Armature'].edit_bones['arm.L'].select
bpy.data.objects['C2'].location.z = bpy.data.armatures['Armature'].edit_bones['L_hand'].select
bpy.ops.armature.select_all(action='DESELECT')
bpy.ops.armature.select_similar(type='DIRECTION', threshold=1.0)
bpy.data.objects['C2'].rotation_euler.x = bpy.data.armatures['Armature'].edit_bones['arm.R'].select
bpy.data.objects['C2'].rotation_euler.y = bpy.data.armatures['Armature'].edit_bones['L_hand'].select
bpy.ops.armature.select_all(action='DESELECT')
bpy.data.armatures['Armature'].edit_bones.active = bpy.data.armatures['Armature'].edit_bones['L_hand']
bpy.ops.armature.select_similar(type='PREFIX')
bpy.data.objects['C2'].rotation_euler.z = bpy.data.armatures['Armature'].edit_bones['R_hand'].select
bpy.data.objects['C2'].scale.x = bpy.data.armatures['Armature'].edit_bones['L_hand'].select
bpy.ops.armature.select_all(action='DESELECT')
bpy.data.armatures['Armature'].edit_bones.active = bpy.data.armatures['Armature'].edit_bones['arm.L']
bpy.ops.armature.select_similar(type='SUFFIX')
bpy.data.objects['C2'].scale.y = bpy.data.armatures['Armature'].edit_bones['leg.L'].select
bpy.data.objects['C2'].scale.z = bpy.data.armatures['Armature'].edit_bones['arm.R'].select
bpy.ops.armature.select_all(action='DESELECT')
bpy.ops.armature.select_similar(type='PREFIX')
bpy.data.objects['C3'].location.x = bpy.data.armatures['Armature'].edit_bones['arm.L'].select
bpy.ops.armature.select_all(action='DESELECT')
bpy.data.armatures['Armature'].edit_bones.active = bpy.data.armatures['Armature'].edit_bones['Bone']
bpy.ops.armature.select_similar(type='CHILDREN')
bpy.data.objects['C3'].location.y = bpy.data.armatures['Armature'].edit_bones['L_hand'].select
bpy.data.objects['C3'].location.z = bpy.data.armatures['Armature'].edit_bones['Bone'].select
bpy.data.objects['C3'].rotation_euler.x = bpy.data.armatures['Armature'].edit_bones['leg.L'].select
bpy.ops.armature.select_all(action='DESELECT')
bpy.ops.armature.select_similar(type='CHILDREN_IMMEDIATE')
bpy.data.objects['C3'].rotation_euler.y = bpy.data.armatures['Armature'].edit_bones['L_hand'].select
bpy.data.objects['C3'].rotation_euler.z = bpy.data.armatures['Armature'].edit_bones['arm.R'].select
bpy.ops.armature.select_all(action='DESELECT')
bpy.data.armatures['Armature'].edit_bones.active = bpy.data.armatures['Armature'].edit_bones['spine']
bpy.ops.armature.select_similar(type='SIBLINGS')
bpy.data.objects['C3'].scale.x = bpy.data.armatures['Armature'].edit_bones['arm.R'].select
bpy.data.objects['C3'].scale.y = bpy.data.armatures['Armature'].edit_bones['Bone'].select
bpy.data.objects['C3'].scale.z = bpy.data.armatures['Armature'].edit_bones['spine'].select
bpy.ops.armature.select_all(action='DESELECT')
bpy.data.armatures['Armature'].edit_bones.active = bpy.data.armatures['Armature'].edit_bones['Bone']
bpy.ops.armature.select_similar(type='SIBLINGS')
bpy.data.objects['C4'].location.x = bpy.data.armatures['Armature'].edit_bones['spine'].select
bpy.ops.armature.select_all(action='DESELECT')
bpy.data.armatures['Armature'].edit_bones.active = bpy.data.armatures['Armature'].edit_bones['arm.L']
bpy.ops.armature.select_similar(type='SHAPE')
bpy.data.objects['C4'].location.y = bpy.data.armatures['Armature'].edit_bones['leg.L'].select
bpy.data.objects['C4'].location.z = bpy.data.armatures['Armature'].edit_bones['spine'].select
bpy.data.objects['C4'].rotation_euler.x = bpy.data.armatures['Armature'].edit_bones['Bone'].select
bpy.ops.armature.select_all(action='DESELECT')
bpy.data.armatures['Armature'].edit_bones.active = bpy.data.armatures['Armature'].edit_bones['Bone']
bpy.ops.armature.select_similar(type='SHAPE')
bpy.data.objects['C4'].rotation_euler.y = bpy.data.armatures['Armature'].edit_bones['arm.R'].select
bpy.data.objects['C4'].rotation_euler.z = bpy.data.armatures['Armature'].edit_bones['spine'].select
bpy.ops.armature.select_all(action='DESELECT')
bpy.data.armatures['Armature'].edit_bones['leg.L'].hide = True
bpy.ops.armature.select_similar(type='COLOR')
bpy.data.objects['C5'].location.x = bpy.data.armatures['Armature'].edit_bones['spine'].select
bpy.data.objects['C5'].location.y = bpy.data.armatures['Armature'].edit_bones['leg.L'].select
bpy.data.armatures['Armature'].edit_bones['leg.L'].hide = False
bpy.ops.armature.select_all(action='DESELECT')
bpy.data.armatures['Armature'].edit_bones['arm.L'].select = True
bpy.data.armatures['Armature'].edit_bones['arm.L'].select_head = True
bpy.data.armatures['Armature'].edit_bones['arm.L'].select_tail = True
bpy.ops.armature.assign_to_collection(new_collection_name='Arms')
bpy.ops.armature.select_all(action='DESELECT')
bpy.data.armatures['Armature'].edit_bones['spine'].select = True
bpy.data.armatures['Armature'].edit_bones['spine'].select_head = True
bpy.data.armatures['Armature'].edit_bones['spine'].select_tail = True
bpy.ops.armature.assign_to_collection(collection_index=0)
bpy.ops.armature.select_all(action='DESELECT')
bpy.data.armatures['Armature'].edit_bones.active = bpy.data.armatures['Armature'].edit_bones['arm.L']
bpy.ops.armature.select_similar(type='BONE_COLLECTION')
bpy.data.objects['C5'].location.z = bpy.data.armatures['Armature'].edit_bones['spine'].select
bpy.data.objects['C5'].rotation_euler.x = bpy.data.armatures['Armature'].edit_bones['arm.R'].select
bpy.data.objects['C5'].rotation_euler.y = bpy.data.armatures['Armature'].edit_bones['arm.L'].select
bpy.data.armatures['Armature'].edit_bones['spine'].select = True
bpy.data.armatures['Armature'].edit_bones.active = None
bpy.ops.armature.select_similar(type='LENGTH')
bpy.ops.armature.select_mirror()
bpy.data.objects['C5'].rotation_euler.z = bpy.data.armatures['Armature'].edit_bones['spine'].select
bpy.ops.object.mode_set(mode='OBJECT')
