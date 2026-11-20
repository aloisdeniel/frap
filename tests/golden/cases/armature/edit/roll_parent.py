# WP 6.2, armature.calculate_roll (chain tangents, global axes, negative types, axis_flip,
# axis_only, the active bone, the 3D cursor), roll_clear, parent_set (CONNECTED moves the
# selection with its children, OFFSET keeps it; only the active bone selected connects it to
# its parent; no active bone is an error), parent_clear (CLEAR, DISCONNECT),
# switch_direction, bone_primitive_add (UP / AXES / 3D_CURSOR / WORLD space, length,
# use_deform, the bone is not made active).
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
bpy.ops.object.armature_add(location=(0, 0, 1), rotation=(0.2, 0.1, 0.4))
bpy.ops.object.mode_set(mode='EDIT')
bpy.data.armatures['Armature'].edit_bones['Bone'].tail = (0.3, 0.2, 1)
bpy.ops.armature.extrude_move(TRANSFORM_OT_translate={'value': (1, 0.2, 0.3)})
bpy.ops.armature.extrude_move(TRANSFORM_OT_translate={'value': (0.2, 1, 0.1)})
bpy.ops.armature.select_all(action='SELECT')
bpy.ops.armature.calculate_roll(type='POS_X')
bpy.data.objects['C0'].location.x = bpy.data.armatures['Armature'].edit_bones['Bone'].roll
bpy.data.objects['C0'].location.y = bpy.data.armatures['Armature'].edit_bones['Bone.001'].roll
bpy.data.objects['C0'].location.z = bpy.data.armatures['Armature'].edit_bones['Bone.002'].roll
bpy.ops.armature.calculate_roll(type='NEG_Z', axis_only=True)
bpy.data.objects['C0'].rotation_euler.x = bpy.data.armatures['Armature'].edit_bones['Bone'].roll
bpy.data.objects['C0'].rotation_euler.y = bpy.data.armatures['Armature'].edit_bones['Bone.001'].roll
bpy.data.objects['C0'].rotation_euler.z = bpy.data.armatures['Armature'].edit_bones['Bone.002'].roll
bpy.ops.armature.calculate_roll(type='GLOBAL_NEG_Y')
bpy.data.objects['C0'].scale.x = bpy.data.armatures['Armature'].edit_bones['Bone'].roll
bpy.data.objects['C0'].scale.y = bpy.data.armatures['Armature'].edit_bones['Bone.001'].roll
bpy.data.objects['C0'].scale.z = bpy.data.armatures['Armature'].edit_bones['Bone.002'].roll
bpy.ops.armature.calculate_roll(type='GLOBAL_POS_Z', axis_flip=True)
bpy.data.objects['C1'].location.x = bpy.data.armatures['Armature'].edit_bones['Bone.001'].roll
bpy.context.scene.cursor.location = (2, -1, 0.5)
bpy.ops.armature.calculate_roll(type='CURSOR')
bpy.data.objects['C1'].location.y = bpy.data.armatures['Armature'].edit_bones['Bone.001'].roll
bpy.ops.armature.calculate_roll(type='CURSOR', axis_flip=True)
bpy.data.objects['C1'].location.z = bpy.data.armatures['Armature'].edit_bones['Bone'].roll
bpy.data.armatures['Armature'].edit_bones['Bone.002'].roll = 1.2
bpy.data.armatures['Armature'].edit_bones.active = bpy.data.armatures['Armature'].edit_bones['Bone.002']
bpy.ops.armature.calculate_roll(type='ACTIVE')
bpy.data.objects['C1'].rotation_euler.x = bpy.data.armatures['Armature'].edit_bones['Bone'].roll
bpy.data.objects['C1'].rotation_euler.y = bpy.data.armatures['Armature'].edit_bones['Bone.001'].roll
bpy.ops.armature.roll_clear(roll=0.5)
bpy.data.objects['C1'].rotation_euler.z = bpy.data.armatures['Armature'].edit_bones['Bone.001'].roll
bpy.ops.armature.select_all(action='DESELECT')
bpy.data.armatures['Armature'].edit_bones['Bone.002'].select = True
bpy.data.armatures['Armature'].edit_bones['Bone.002'].select_head = True
bpy.data.armatures['Armature'].edit_bones['Bone.002'].select_tail = True
bpy.data.armatures['Armature'].edit_bones.active = bpy.data.armatures['Armature'].edit_bones['Bone']
bpy.ops.armature.parent_set(type='OFFSET')
bpy.data.armatures['Armature'].edit_bones['Bone.002'].parent.name
bpy.data.objects['C2'].location = bpy.data.armatures['Armature'].edit_bones['Bone.002'].head
bpy.ops.armature.parent_set(type='CONNECTED')
bpy.data.objects['C2'].rotation_euler = bpy.data.armatures['Armature'].edit_bones['Bone.002'].head
bpy.data.objects['C2'].scale = bpy.data.armatures['Armature'].edit_bones['Bone.002'].tail
bpy.ops.armature.parent_clear(type='DISCONNECT')
bpy.data.objects['C3'].location.x = bpy.data.armatures['Armature'].edit_bones['Bone.002'].use_connect
bpy.data.armatures['Armature'].edit_bones['Bone.002'].parent.name
bpy.ops.armature.parent_clear(type='CLEAR')
bpy.data.armatures['Armature'].edit_bones['Bone.002'].parent.name
bpy.data.armatures['Armature'].edit_bones.active = bpy.data.armatures['Armature'].edit_bones['Bone.002']
bpy.data.armatures['Armature'].edit_bones['Bone.002'].parent = bpy.data.armatures['Armature'].edit_bones['Bone.001']
bpy.ops.armature.select_all(action='DESELECT')
bpy.data.armatures['Armature'].edit_bones['Bone.002'].select = True
bpy.ops.armature.parent_set()
bpy.data.objects['C3'].location.y = bpy.data.armatures['Armature'].edit_bones['Bone.002'].use_connect
bpy.data.objects['C3'].rotation_euler = bpy.data.armatures['Armature'].edit_bones['Bone.002'].head
bpy.ops.armature.select_all(action='SELECT')
bpy.ops.armature.switch_direction()
bpy.data.armatures['Armature'].edit_bones['Bone'].parent.name
bpy.data.armatures['Armature'].edit_bones['Bone.002'].parent
bpy.data.objects['C3'].scale = bpy.data.armatures['Armature'].edit_bones['Bone'].head
bpy.data.objects['C4'].location = bpy.data.armatures['Armature'].edit_bones['Bone.002'].tail
bpy.data.objects['C4'].rotation_euler.x = bpy.data.armatures['Armature'].edit_bones['Bone'].use_connect
bpy.data.armatures['Armature'].edit_bones.active = None
bpy.ops.armature.parent_set()
bpy.ops.armature.bone_primitive_add()
bpy.data.armatures['Armature'].edit_bones.active
bpy.data.objects['C4'].scale = bpy.data.armatures['Armature'].edit_bones['Bone.003'].tail
bpy.data.objects['C4'].rotation_euler.y = bpy.data.armatures['Armature'].edit_bones['Bone.003'].roll
bpy.data.objects['C4'].rotation_euler.z = bpy.data.armatures['Armature'].edit_bones['Bone.002'].select
bpy.ops.armature.bone_primitive_add(name='W', space='WORLD', length=2, use_deform=False)
bpy.data.objects['C5'].location = bpy.data.armatures['Armature'].edit_bones['W'].tail
bpy.data.objects['C5'].rotation_euler.x = bpy.data.armatures['Armature'].edit_bones['W'].roll
bpy.data.objects['C5'].rotation_euler.y = bpy.data.armatures['Armature'].edit_bones['W'].use_deform
bpy.data.objects['C5'].rotation_euler.z = bpy.data.armatures['Armature'].edit_bones['W'].head_radius
bpy.ops.armature.bone_primitive_add(name='A', align='AXES')
bpy.data.objects['C5'].scale = bpy.data.armatures['Armature'].edit_bones['A'].tail
bpy.ops.armature.bone_primitive_add(name='AW', align='AXES', space='WORLD')
bpy.data.objects['C6'].location = bpy.data.armatures['Armature'].edit_bones['AW'].tail
bpy.data.objects['C6'].rotation_euler.x = bpy.data.armatures['Armature'].edit_bones['AW'].roll
bpy.context.scene.cursor.rotation_euler = (0.3, 0.6, -0.2)
bpy.ops.armature.bone_primitive_add(name='K', align='3D_CURSOR')
bpy.data.objects['C6'].scale = bpy.data.armatures['Armature'].edit_bones['K'].tail
bpy.data.objects['C6'].rotation_euler.y = bpy.data.armatures['Armature'].edit_bones['K'].roll
