# WP 6.2, armature.extrude / extrude_move / duplicate / duplicate_move / subdivide / delete /
# select_all and the armature Edit Mode transform handler: new bones' names, parents,
# connection, selection and active bone; a zero-length extruded bone gets its radii and
# envelope from its new length; a moved tip drags the connected child; rolls follow a
# rotation; subdivide cuts into equal connected pieces named along the chain; delete
# re-parents children and removes the pose channels.
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
bpy.ops.object.armature_add(rotation=(0, 0.3, 0))
bpy.data.objects['Armature'].scale = (2, 1, 1)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.armature.extrude_move(TRANSFORM_OT_translate={'value': (0.5, 0, 1)})
bpy.context.active_bone.name
bpy.data.objects['C0'].location = bpy.data.armatures['Armature'].edit_bones['Bone.001'].tail
bpy.data.objects['C0'].rotation_euler.x = bpy.data.armatures['Armature'].edit_bones['Bone.001'].head_radius
bpy.data.objects['C0'].rotation_euler.y = bpy.data.armatures['Armature'].edit_bones['Bone.001'].tail_radius
bpy.data.objects['C0'].rotation_euler.z = bpy.data.armatures['Armature'].edit_bones['Bone.001'].envelope_distance
bpy.data.objects['C0'].scale.x = bpy.data.armatures['Armature'].edit_bones['Bone.001'].roll
bpy.data.objects['C0'].scale.y = bpy.data.armatures['Armature'].edit_bones['Bone.001'].use_connect
bpy.data.objects['C0'].scale.z = bpy.data.armatures['Armature'].edit_bones['Bone'].select_tail
bpy.ops.armature.extrude(forked=True)
bpy.ops.transform.translate(value=(0, 1, 0))
bpy.data.objects['C1'].location = bpy.data.armatures['Armature'].edit_bones['Bone.002'].tail
bpy.data.objects['C1'].rotation_euler.x = bpy.data.armatures['Armature'].edit_bones['Bone.002'].head_radius
bpy.data.objects['C1'].rotation_euler.y = bpy.data.armatures['Armature'].edit_bones['Bone.002'].tail_radius
bpy.data.objects['C1'].rotation_euler.z = bpy.data.armatures['Armature'].edit_bones['Bone.001'].tail_radius
bpy.ops.armature.select_all(action='DESELECT')
bpy.data.armatures['Armature'].edit_bones['Bone.001'].select_tail = True
bpy.ops.transform.translate(value=(0, 0, 0.5))
bpy.data.objects['C1'].scale = bpy.data.armatures['Armature'].edit_bones['Bone.002'].head
bpy.data.objects['C2'].rotation_euler.x = bpy.data.armatures['Armature'].edit_bones['Bone.001'].roll
bpy.data.objects['C2'].rotation_euler.y = bpy.data.armatures['Armature'].edit_bones['Bone.001'].envelope_distance
bpy.data.objects['C2'].rotation_euler.z = bpy.data.armatures['Armature'].edit_bones['Bone.002'].head_radius
bpy.ops.armature.select_all(action='SELECT')
bpy.ops.transform.rotate(value=0.4, orient_axis='Y')
bpy.data.objects['C2'].location = bpy.data.armatures['Armature'].edit_bones['Bone.002'].tail
bpy.data.objects['C2'].scale.x = bpy.data.armatures['Armature'].edit_bones['Bone'].roll
bpy.data.objects['C2'].scale.y = bpy.data.armatures['Armature'].edit_bones['Bone.001'].roll
bpy.data.objects['C2'].scale.z = bpy.data.armatures['Armature'].edit_bones['Bone.002'].roll
bpy.ops.transform.resize(value=(1, 1.5, 0.5))
bpy.data.objects['C3'].location = bpy.data.armatures['Armature'].edit_bones['Bone.002'].tail
bpy.data.objects['C3'].rotation_euler.x = bpy.data.armatures['Armature'].edit_bones['Bone.002'].tail_radius
bpy.data.objects['C3'].rotation_euler.y = bpy.data.armatures['Armature'].edit_bones['Bone'].envelope_distance
bpy.ops.armature.select_all(action='DESELECT')
bpy.data.armatures['Armature'].edit_bones['Bone.001'].select = True
bpy.data.armatures['Armature'].edit_bones['Bone.001'].select_head = True
bpy.data.armatures['Armature'].edit_bones['Bone.001'].select_tail = True
bpy.ops.armature.duplicate_move(TRANSFORM_OT_translate={'value': (1, 0, 0)})
bpy.context.active_bone.name
bpy.data.armatures['Armature'].edit_bones['Bone.003'].parent.name
bpy.data.objects['C3'].scale = bpy.data.armatures['Armature'].edit_bones['Bone.003'].head
bpy.data.objects['C4'].location.x = bpy.data.armatures['Armature'].edit_bones['Bone.003'].use_connect
bpy.data.objects['C4'].location.y = bpy.data.armatures['Armature'].edit_bones['Bone.001'].select
bpy.data.objects['C4'].location.z = bpy.data.armatures['Armature'].edit_bones['Bone.003'].select
bpy.ops.armature.subdivide(number_cuts=2)
bpy.data.armatures['Armature'].edit_bones['Bone.005'].parent.name
bpy.data.armatures['Armature'].edit_bones['Bone.004'].parent.name
bpy.data.objects['C4'].rotation_euler = bpy.data.armatures['Armature'].edit_bones['Bone.003'].tail
bpy.data.objects['C4'].scale = bpy.data.armatures['Armature'].edit_bones['Bone.004'].tail
bpy.data.objects['C5'].location = bpy.data.armatures['Armature'].edit_bones['Bone.005'].head
bpy.data.objects['C5'].rotation_euler.x = bpy.data.armatures['Armature'].edit_bones['Bone.004'].head_radius
bpy.data.objects['C5'].rotation_euler.y = bpy.data.armatures['Armature'].edit_bones['Bone.003'].tail_radius
bpy.ops.armature.select_all(action='DESELECT')
bpy.ops.armature.delete()
bpy.ops.armature.extrude()
bpy.data.armatures['Armature'].edit_bones['Bone.004'].select = True
bpy.ops.armature.delete()
bpy.data.armatures['Armature'].edit_bones['Bone.004']
bpy.data.armatures['Armature'].edit_bones['Bone.005'].parent.name
bpy.data.armatures['Armature'].edit_bones['Bone.005'].use_connect
bpy.ops.armature.select_all(action='TOGGLE')
bpy.data.objects['C5'].scale.x = bpy.data.armatures['Armature'].edit_bones['Bone'].select
bpy.ops.armature.select_all(action='TOGGLE')
bpy.data.objects['C5'].scale.y = bpy.data.armatures['Armature'].edit_bones['Bone'].select
bpy.data.armatures['Armature'].edit_bones['Bone'].select = True
bpy.ops.armature.select_all(action='INVERT')
bpy.data.objects['C5'].scale.z = bpy.data.armatures['Armature'].edit_bones['Bone.002'].select_tail
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Armature'].pose.bones['Bone.004']
bpy.data.objects['Armature'].pose.bones['Bone.005'].name
bpy.data.objects['C6'].matrix_world = bpy.data.armatures['Armature'].bones['Bone.002'].matrix_local
bpy.data.objects['C7'].matrix_world = bpy.data.armatures['Armature'].bones['Bone.005'].matrix_local
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.armature.select_all(action='DESELECT')
bpy.ops.armature.extrude()
bpy.ops.armature.duplicate()
