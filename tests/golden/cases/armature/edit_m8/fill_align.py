# WP 8.3.10, armature.fill: one joint to the 3D cursor (object space of a moved, rotated
# armature), two tails ordered by the active bone or the cursor distance, a head and a tail
# (connected child), the new bone's collections, active bone and selection; its errors (no
# joints, same bone, too many points). armature.align: selected bones take the active bone's
# direction and roll keeping their length, connected children follow, a lone active bone
# aligns to its parent, no active bone is an error.
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
bpy.data.objects.new('N1', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N1'])
bpy.data.objects.new('N2', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N2'])
bpy.ops.object.armature_add(location=(1, 0, 0), rotation=(0, 0, 0.5))
bpy.context.scene.cursor.location = (2, 3, 1)
bpy.ops.object.mode_set(mode='EDIT')
bpy.data.armatures['Armature'].edit_bones.new('A')
bpy.data.armatures['Armature'].edit_bones['A'].head = (1, 0, 0)
bpy.data.armatures['Armature'].edit_bones['A'].tail = (1, 0, 1)
bpy.data.armatures['Armature'].edit_bones['A'].roll = 0.3
bpy.data.armatures['Armature'].edit_bones.new('B')
bpy.data.armatures['Armature'].edit_bones['B'].head = (2, 0, 0)
bpy.data.armatures['Armature'].edit_bones['B'].tail = (2, 1, 2)
bpy.data.armatures['Armature'].edit_bones['B'].roll = -0.7
bpy.data.armatures['Armature'].edit_bones.new('B2')
bpy.data.armatures['Armature'].edit_bones['B2'].head = (2, 1, 2)
bpy.data.armatures['Armature'].edit_bones['B2'].tail = (2, 1, 3)
bpy.data.armatures['Armature'].edit_bones['B2'].parent = bpy.data.armatures['Armature'].edit_bones['B']
bpy.data.armatures['Armature'].edit_bones['B2'].use_connect = True
bpy.ops.armature.select_all(action='DESELECT')
bpy.ops.armature.fill()
bpy.data.armatures['Armature'].edit_bones['A'].select_tail = True
bpy.ops.armature.fill()
bpy.data.objects['N0'].name = bpy.context.active_bone.name
bpy.data.objects['C0'].location = bpy.data.armatures['Armature'].edit_bones['Bone.001'].head
bpy.data.objects['C0'].rotation_euler = bpy.data.armatures['Armature'].edit_bones['Bone.001'].tail
bpy.data.objects['C0'].scale.x = bpy.data.armatures['Armature'].edit_bones['Bone.001'].select_tail
bpy.data.objects['C0'].scale.y = bpy.data.armatures['Armature'].edit_bones['A'].select_tail
bpy.data.objects['C0'].scale.z = bpy.data.armatures['Armature'].edit_bones['Bone.001'].use_connect
bpy.ops.armature.select_all(action='DESELECT')
bpy.data.armatures['Armature'].edit_bones['A'].select_tail = True
bpy.data.armatures['Armature'].edit_bones['B'].select_tail = True
bpy.ops.armature.fill()
bpy.data.objects['N1'].name = bpy.data.armatures['Armature'].edit_bones['Bone.002'].parent.name
bpy.data.objects['C1'].location = bpy.data.armatures['Armature'].edit_bones['Bone.002'].head
bpy.data.objects['C1'].rotation_euler = bpy.data.armatures['Armature'].edit_bones['Bone.002'].tail
bpy.data.objects['C1'].scale.x = bpy.data.armatures['Armature'].edit_bones['Bone.002'].use_connect
bpy.ops.armature.select_all(action='DESELECT')
bpy.data.armatures['Armature'].edit_bones.active = bpy.data.armatures['Armature'].edit_bones['A']
bpy.data.armatures['Armature'].edit_bones['A'].select_tail = True
bpy.data.armatures['Armature'].edit_bones['B2'].select_tail = True
bpy.ops.armature.fill()
bpy.data.objects['N2'].name = bpy.data.armatures['Armature'].edit_bones['Bone.003'].parent.name
bpy.data.objects['C2'].location = bpy.data.armatures['Armature'].edit_bones['Bone.003'].head
bpy.data.objects['C2'].rotation_euler = bpy.data.armatures['Armature'].edit_bones['Bone.003'].tail
bpy.ops.armature.select_all(action='DESELECT')
bpy.data.armatures['Armature'].edit_bones['A'].select_tail = True
bpy.data.armatures['Armature'].edit_bones['B'].select_head = True
bpy.ops.armature.fill()
bpy.data.objects['C3'].location = bpy.data.armatures['Armature'].edit_bones['Bone.004'].head
bpy.data.objects['C3'].rotation_euler = bpy.data.armatures['Armature'].edit_bones['Bone.004'].tail
bpy.data.objects['C3'].scale.x = bpy.data.armatures['Armature'].edit_bones['Bone.004'].use_connect
bpy.data.armatures['Armature'].edit_bones['Bone.004'].parent.name
bpy.ops.armature.select_all(action='DESELECT')
bpy.data.armatures['Armature'].edit_bones['A'].select_head = True
bpy.data.armatures['Armature'].edit_bones['A'].select_tail = True
bpy.ops.armature.fill()
bpy.ops.armature.select_all(action='DESELECT')
bpy.data.armatures['Armature'].edit_bones['A'].select_head = True
bpy.data.armatures['Armature'].edit_bones['B'].select_head = True
bpy.data.armatures['Armature'].edit_bones['B2'].select_tail = True
bpy.ops.armature.fill()
bpy.ops.armature.select_all(action='DESELECT')
bpy.data.armatures['Armature'].edit_bones['A'].select = True
bpy.data.armatures['Armature'].edit_bones['A'].select_head = True
bpy.data.armatures['Armature'].edit_bones['A'].select_tail = True
bpy.ops.armature.assign_to_collection(new_collection_name='Fill')
bpy.ops.armature.select_all(action='DESELECT')
bpy.data.armatures['Armature'].edit_bones['A'].select_head = True
bpy.ops.armature.fill()
bpy.data.armatures['Armature'].edit_bones['Bone.005'].collections['Fill']
bpy.ops.armature.select_all(action='DESELECT')
bpy.data.armatures['Armature'].edit_bones.active = bpy.data.armatures['Armature'].edit_bones['A']
bpy.data.armatures['Armature'].edit_bones['A'].select = True
bpy.data.armatures['Armature'].edit_bones['A'].select_head = True
bpy.data.armatures['Armature'].edit_bones['A'].select_tail = True
bpy.data.armatures['Armature'].edit_bones['B'].select = True
bpy.data.armatures['Armature'].edit_bones['B'].select_head = True
bpy.data.armatures['Armature'].edit_bones['B'].select_tail = True
bpy.ops.armature.align()
bpy.data.objects['C4'].location = bpy.data.armatures['Armature'].edit_bones['B'].tail
bpy.data.objects['C4'].rotation_euler.x = bpy.data.armatures['Armature'].edit_bones['B'].roll
bpy.data.objects['C4'].scale = bpy.data.armatures['Armature'].edit_bones['B2'].tail
bpy.data.objects['C5'].location = bpy.data.armatures['Armature'].edit_bones['B2'].head
bpy.ops.armature.select_all(action='DESELECT')
bpy.data.armatures['Armature'].edit_bones.active = bpy.data.armatures['Armature'].edit_bones['B2']
bpy.data.armatures['Armature'].edit_bones['B2'].roll = 1.1
bpy.data.armatures['Armature'].edit_bones['B2'].tail = (3, 2, 4)
bpy.data.armatures['Armature'].edit_bones['B2'].select = True
bpy.data.armatures['Armature'].edit_bones['B2'].select_tail = True
bpy.ops.armature.align()
bpy.data.objects['C5'].rotation_euler = bpy.data.armatures['Armature'].edit_bones['B2'].tail
bpy.data.objects['C5'].scale.x = bpy.data.armatures['Armature'].edit_bones['B2'].roll
bpy.data.armatures['Armature'].edit_bones.active = None
bpy.ops.armature.align()
bpy.data.armatures['Armature'].edit_bones.active = bpy.data.armatures['Armature'].edit_bones['A']
bpy.ops.armature.select_all(action='DESELECT')
bpy.ops.armature.align()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['C6'].matrix_world = bpy.data.armatures['Armature'].bones['Bone.001'].matrix_local
bpy.data.objects['C7'].matrix_world = bpy.data.armatures['Armature'].bones['B'].matrix_local
