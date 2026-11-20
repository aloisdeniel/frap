# WP 6.2, armature.symmetrize: side-named bones get X-mirrored copies (.L -> .R) parented to
# the mirror of their parent or the same centre parent, with mirrored bone collections
# (created when missing), existing mirrors are updated (direction picks the source when both
# sides are selected), unflippable bones are deselected; Armature.use_mirror_x mirrors edit
# transforms, extrudes and roll changes onto the X mirror bone; duplicate(do_flip_names).
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
bpy.ops.object.armature_add()
bpy.ops.object.mode_set(mode='EDIT')
bpy.data.armatures['Armature'].edit_bones.new('arm.L')
bpy.data.armatures['Armature'].edit_bones['arm.L'].head = (0.2, 0, 1)
bpy.data.armatures['Armature'].edit_bones['arm.L'].tail = (1, 0.2, 1.1)
bpy.data.armatures['Armature'].edit_bones['arm.L'].roll = 0.3
bpy.data.armatures['Armature'].edit_bones['arm.L'].parent = bpy.data.armatures['Armature'].edit_bones['Bone']
bpy.data.armatures['Armature'].edit_bones.new('hand.L')
bpy.data.armatures['Armature'].edit_bones['hand.L'].head = (1, 0.2, 1.1)
bpy.data.armatures['Armature'].edit_bones['hand.L'].tail = (1.4, 0.2, 1.0)
bpy.data.armatures['Armature'].edit_bones['hand.L'].parent = bpy.data.armatures['Armature'].edit_bones['arm.L']
bpy.data.armatures['Armature'].edit_bones['hand.L'].use_connect = True
bpy.data.armatures['Armature'].edit_bones['hand.L'].use_deform = False
bpy.data.armatures['Armature'].edit_bones.new('leg.R')
bpy.data.armatures['Armature'].edit_bones['leg.R'].head = (-0.3, 0, 0)
bpy.data.armatures['Armature'].edit_bones['leg.R'].tail = (-0.4, -0.2, -1)
bpy.data.armatures['Armature'].edit_bones.new('leg.L')
bpy.data.armatures['Armature'].edit_bones['leg.L'].head = (0.5, 0, 0)
bpy.data.armatures['Armature'].edit_bones['leg.L'].tail = (0.5, 0, -1)
bpy.ops.armature.select_all(action='SELECT')
bpy.ops.armature.symmetrize()
bpy.data.armatures['Armature'].edit_bones['arm.R'].parent.name
bpy.data.armatures['Armature'].edit_bones['hand.R'].parent.name
bpy.data.objects['C0'].location = bpy.data.armatures['Armature'].edit_bones['arm.R'].head
bpy.data.objects['C0'].rotation_euler = bpy.data.armatures['Armature'].edit_bones['hand.R'].tail
bpy.data.objects['C0'].scale.x = bpy.data.armatures['Armature'].edit_bones['arm.R'].roll
bpy.data.objects['C0'].scale.y = bpy.data.armatures['Armature'].edit_bones['hand.R'].use_deform
bpy.data.objects['C0'].scale.z = bpy.data.armatures['Armature'].edit_bones['hand.R'].use_connect
bpy.data.objects['C1'].location = bpy.data.armatures['Armature'].edit_bones['leg.L'].tail
bpy.data.objects['C1'].rotation_euler = bpy.data.armatures['Armature'].edit_bones['leg.R'].tail
bpy.data.objects['C1'].scale.x = bpy.data.armatures['Armature'].edit_bones['Bone'].select
bpy.data.objects['C1'].scale.y = bpy.data.armatures['Armature'].edit_bones['arm.L'].select
bpy.data.objects['C1'].scale.z = bpy.data.armatures['Armature'].edit_bones['arm.R'].select
bpy.data.armatures['Armature'].edit_bones['arm.R'].collections['Bones'].name
bpy.ops.armature.select_all(action='SELECT')
bpy.ops.armature.symmetrize(direction='POSITIVE_X')
bpy.data.objects['C2'].location = bpy.data.armatures['Armature'].edit_bones['leg.R'].tail
bpy.data.objects['C2'].rotation_euler = bpy.data.armatures['Armature'].edit_bones['arm.L'].head
bpy.data.armatures['Armature'].use_mirror_x = True
bpy.ops.armature.select_all(action='DESELECT')
bpy.data.armatures['Armature'].edit_bones['hand.L'].select_tail = True
bpy.ops.transform.translate(value=(0.2, 0.1, 0.3))
bpy.data.objects['C2'].scale = bpy.data.armatures['Armature'].edit_bones['hand.R'].tail
bpy.data.objects['C3'].location.x = bpy.data.armatures['Armature'].edit_bones['hand.R'].roll
bpy.data.objects['C3'].location.y = bpy.data.armatures['Armature'].edit_bones['hand.L'].roll
bpy.ops.armature.extrude_move(TRANSFORM_OT_translate={'value': (0.3, 0, 0)})
bpy.data.armatures['Armature'].edit_bones['hand.R.001'].parent.name
bpy.data.objects['C3'].rotation_euler = bpy.data.armatures['Armature'].edit_bones['hand.R.001'].tail
bpy.data.objects['C3'].scale = bpy.data.armatures['Armature'].edit_bones['hand.L.001'].tail
bpy.ops.armature.select_all(action='DESELECT')
bpy.data.armatures['Armature'].edit_bones['arm.L'].select = True
bpy.ops.armature.roll_clear(roll=0.25)
bpy.data.objects['C3'].location.z = bpy.data.armatures['Armature'].edit_bones['arm.R'].roll
bpy.data.armatures['Armature'].use_mirror_x = False
bpy.ops.armature.select_all(action='DESELECT')
bpy.data.armatures['Armature'].edit_bones['leg.L'].select = True
bpy.data.armatures['Armature'].edit_bones['leg.L'].select_head = True
bpy.data.armatures['Armature'].edit_bones['leg.L'].select_tail = True
bpy.ops.armature.duplicate(do_flip_names=True)
bpy.data.armatures['Armature'].edit_bones['leg.L.001']
bpy.data.armatures['Armature'].edit_bones['Bone'].select = True
bpy.ops.armature.duplicate(do_flip_names=True)
bpy.data.armatures['Armature'].edit_bones['Bone.001']
bpy.data.objects['C4'].location.x = bpy.data.armatures['Armature'].edit_bones['leg.L.002'].select
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.armatures['Armature'].collections_all['Bones'].name
bpy.data.objects['C4'].matrix_world = bpy.data.armatures['Armature'].bones['hand.R'].matrix_local
