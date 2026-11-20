# WP 6.2, object.armature_add: object and data both requested as 'Armature' and uniquified
# independently; one bone 'Bone' from the origin to (0, 0, radius), in the default bone
# collection 'Bones' (active), active, only its tail selected; the pose has one QUATERNION
# channel, not selected. Placement like the primitives (cursor, align, set rotation wins),
# scale ignored. enter_editmode=True stays in Edit Mode without a pose. Values are captured
# into the empty C0..C3 (location / rotation_euler / scale slots, matrix_world).
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
bpy.ops.object.armature_add()
bpy.data.armatures['Armature'].bones['Bone']
bpy.data.armatures['Armature'].bones['Bone.001']
bpy.data.armatures['Armature'].collections_all['Bones']
bpy.data.armatures['Armature'].collections['Bones']
bpy.data.armatures['Armature'].collections.active.name
bpy.data.armatures['Armature'].bones.active.name
bpy.data.objects['Armature'].pose.bones['Bone']
bpy.data.objects['C0'].matrix_world = bpy.data.armatures['Armature'].bones['Bone'].matrix_local
bpy.data.objects['C1'].location = bpy.data.armatures['Armature'].bones['Bone'].tail_local
bpy.data.objects['C1'].rotation_euler.x = bpy.data.armatures['Armature'].bones['Bone'].length
bpy.data.objects['C1'].rotation_euler.y = bpy.data.armatures['Armature'].bones['Bone'].head_radius
bpy.data.objects['C1'].rotation_euler.z = bpy.data.armatures['Armature'].bones['Bone'].tail_radius
bpy.data.objects['C1'].scale.x = bpy.data.armatures['Armature'].bones['Bone'].envelope_distance
bpy.data.objects['C1'].scale.y = bpy.data.armatures['Armature'].bones['Bone'].envelope_weight
bpy.data.objects['C1'].scale.z = bpy.data.armatures['Armature'].bones['Bone'].bbone_segments
bpy.data.objects['C2'].location.x = bpy.data.objects['Armature'].pose.bones['Bone'].select
bpy.data.objects['C2'].location.y = bpy.data.armatures['Armature'].bones['Bone'].use_deform
bpy.data.objects['C2'].location.z = bpy.data.armatures['Armature'].bones['Bone'].use_connect
bpy.data.objects['C2'].rotation_euler.x = bpy.data.armatures['Armature'].bones['Bone'].use_inherit_rotation
bpy.data.objects['C2'].rotation_euler.y = bpy.data.armatures['Armature'].bones['Bone'].use_local_location
bpy.data.objects['C2'].rotation_euler.z = bpy.data.armatures['Armature'].is_editmode
bpy.data.objects['C2'].scale = bpy.data.objects['Armature'].pose.bones['Bone'].rotation_quaternion.to_euler()
bpy.data.objects['C3'].scale = bpy.data.objects['Armature'].pose.bones['Bone'].scale
bpy.ops.object.mode_set(mode='EDIT')
bpy.data.objects['C3'].location.x = bpy.data.armatures['Armature'].edit_bones['Bone'].select
bpy.data.objects['C3'].location.y = bpy.data.armatures['Armature'].edit_bones['Bone'].select_head
bpy.data.objects['C3'].location.z = bpy.data.armatures['Armature'].edit_bones['Bone'].select_tail
bpy.data.objects['C3'].rotation_euler.x = bpy.data.armatures['Armature'].edit_bones['Bone'].roll
bpy.data.objects['C3'].rotation_euler.y = bpy.data.armatures['Armature'].is_editmode
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.object.armature_add(location=(1, 2, 3), rotation=(0.1, 0.2, 0.3), radius=2.5, scale=(3, 3, 3))
bpy.data.armatures['Armature.001'].bones['Bone']
bpy.data.objects['C0'].location = bpy.data.armatures['Armature.001'].bones['Bone'].tail_local
bpy.ops.object.armature_add(align='CURSOR', enter_editmode=True)
bpy.context.mode
bpy.data.armatures['Armature.002'].edit_bones['Bone']
bpy.data.objects['C0'].scale = bpy.data.armatures['Armature.002'].edit_bones['Bone'].tail
bpy.data.objects['Armature.002'].pose
bpy.ops.object.armature_add()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.object.armature_add()
bpy.data.armatures['Armature.003'].bones['Bone']
