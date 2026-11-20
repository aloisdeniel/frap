# WP 6.2, Armature.edit_bones: new() outside Edit Mode is an error; new bones start at the
# origin with only the tail selected; head/tail/roll/parent/use_connect/length/matrix
# setters (connected bones follow, a connected head snaps to the parent tail); remove()
# re-parents the children; leaving Edit Mode builds the rest matrices in depth-first order
# (Bone.head/tail in the parent's space, matrix, matrix_local, length) and elongates a
# zero-length bone; the pose gets one channel per bone.
# golden: startup=factory compare=objects,context tier=T0
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
bpy.ops.object.armature_add()
bpy.data.armatures['Armature'].edit_bones.new('Arm')
bpy.data.armatures['Armature'].edit_bones.remove(bpy.data.armatures['Armature'].bones['Bone'])
bpy.ops.object.mode_set(mode='EDIT')
bpy.data.armatures['Armature'].edit_bones.new('Arm')
bpy.data.armatures['Armature'].edit_bones.new('Arm')
bpy.data.armatures['Armature'].edit_bones.new('')
bpy.data.objects['C0'].location = bpy.data.armatures['Armature'].edit_bones['Arm'].tail
bpy.data.objects['C0'].rotation_euler.x = bpy.data.armatures['Armature'].edit_bones['Arm'].select
bpy.data.objects['C0'].rotation_euler.y = bpy.data.armatures['Armature'].edit_bones['Arm'].select_tail
bpy.data.objects['C0'].rotation_euler.z = bpy.data.armatures['Armature'].edit_bones['Arm'].head_radius
bpy.data.armatures['Armature'].edit_bones['Arm'].head = (0.5, 0.25, 1)
bpy.data.armatures['Armature'].edit_bones['Arm'].tail = (1.5, 0.5, 1.5)
bpy.data.armatures['Armature'].edit_bones['Arm'].roll = 0.7
bpy.data.armatures['Armature'].edit_bones['Arm'].parent = bpy.data.armatures['Armature'].edit_bones['Bone']
bpy.data.armatures['Armature'].edit_bones['Arm'].use_connect = True
bpy.data.objects['C1'].location = bpy.data.armatures['Armature'].edit_bones['Arm'].head
bpy.data.armatures['Armature'].edit_bones['Arm.001'].head = (-1, 0, 0)
bpy.data.armatures['Armature'].edit_bones['Arm.001'].tail = (-1, 1, 0.5)
bpy.data.armatures['Armature'].edit_bones['Arm.001'].parent = bpy.data.armatures['Armature'].edit_bones['Arm']
bpy.data.armatures['Armature'].edit_bones['Arm.001'].length = 3
bpy.data.objects['C1'].rotation_euler = bpy.data.armatures['Armature'].edit_bones['Arm.001'].tail
bpy.data.armatures['Armature'].edit_bones['Bone'].parent = bpy.data.armatures['Armature'].edit_bones['Arm.001']
bpy.data.armatures['Armature'].edit_bones['Bone'].parent
bpy.data.armatures['Armature'].edit_bones['Bone'].tail = (0, 0, 2)
bpy.data.objects['C1'].scale = bpy.data.armatures['Armature'].edit_bones['Arm'].head
bpy.data.armatures['Armature'].edit_bones['Bone.001'].matrix = bpy.data.armatures['Armature'].edit_bones['Arm'].matrix
bpy.data.objects['C2'].matrix_world = bpy.data.armatures['Armature'].edit_bones['Arm'].matrix
bpy.data.objects['C2'].scale.x = bpy.data.armatures['Armature'].edit_bones['Arm'].length
bpy.data.armatures['Armature'].edit_bones.new('Tmp')
bpy.data.armatures['Armature'].edit_bones['Tmp'].tail = (0, 0, 1)
bpy.data.armatures['Armature'].edit_bones['Bone.001'].parent = bpy.data.armatures['Armature'].edit_bones['Tmp']
bpy.data.armatures['Armature'].edit_bones.remove(bpy.data.armatures['Armature'].edit_bones['Tmp'])
bpy.data.armatures['Armature'].edit_bones['Bone.001'].parent
bpy.data.armatures['Armature'].edit_bones.active = bpy.data.armatures['Armature'].edit_bones['Arm']
bpy.data.armatures['Armature'].edit_bones.active.name
bpy.data.objects['C2'].rotation_euler.z = bpy.data.armatures['Armature'].edit_bones['Arm'].select
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.armatures['Armature'].edit_bones['Arm']
bpy.data.armatures['Armature'].bones[0].name
bpy.data.armatures['Armature'].bones[1].name
bpy.data.armatures['Armature'].bones[2].name
bpy.data.armatures['Armature'].bones[3].name
bpy.data.armatures['Armature'].bones[4]
bpy.data.armatures['Armature'].bones.active.name
bpy.data.objects['Armature'].pose.bones[3].name
bpy.data.objects['C3'].matrix_world = bpy.data.armatures['Armature'].bones['Arm.001'].matrix_local
bpy.data.objects['C3'].scale = bpy.data.armatures['Armature'].bones['Arm.001'].head
bpy.data.objects['C4'].location = bpy.data.armatures['Armature'].bones['Arm.001'].tail
bpy.data.objects['C4'].rotation_euler = bpy.data.armatures['Armature'].bones['Bone'].head
bpy.data.objects['C4'].scale = bpy.data.armatures['Armature'].bones['Bone'].tail
bpy.data.objects['C5'].location = bpy.data.armatures['Armature'].bones['Bone.001'].tail_local
bpy.data.objects['C5'].rotation_euler.x = bpy.data.armatures['Armature'].bones['Arm.001'].length
bpy.data.objects['C5'].rotation_euler.y = bpy.data.armatures['Armature'].bones['Arm'].length
bpy.data.objects['C5'].rotation_euler.z = bpy.data.armatures['Armature'].bones['Bone.001'].length
bpy.data.objects['C5'].scale = bpy.data.armatures['Armature'].bones['Arm.001'].matrix.to_euler()
bpy.data.armatures['Armature'].bones['Arm.001'].parent.name
bpy.data.armatures['Armature'].bones['Arm'].children[0].name
bpy.data.armatures['Armature'].bones['Arm'].use_connect
