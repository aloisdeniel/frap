# WP 8.3.10, armature.extrude_forked (the macro forces forked=True on its extrude step, so
# use_mirror_x gives a _L/_R pair for a bone without a mirror even with
# ARMATURE_OT_extrude={'forked': False}; the translate step moves the new tails), and the new
# operators on two armatures in Edit Mode at once (select_more, hide, reveal, split, dissolve,
# flip_names, select_similar LENGTH across objects, align across objects, fill across objects
# is an error), and their polls outside armature Edit Mode.
# golden: startup=factory compare=objects,context tier=T0 tol=1e-5
import bpy
bpy.data.objects.new('C0', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['C0'])
bpy.data.objects.new('C1', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['C1'])
bpy.data.objects.new('C2', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['C2'])
bpy.ops.armature.select_more()
bpy.ops.armature.dissolve()
bpy.ops.armature.separate()
bpy.ops.armature.flip_names()
bpy.ops.armature.extrude_forked()
bpy.ops.object.armature_add()
bpy.ops.armature.fill()
bpy.ops.object.mode_set(mode='EDIT')
bpy.data.armatures['Armature'].use_mirror_x = True
bpy.ops.armature.select_all(action='SELECT')
bpy.ops.armature.extrude_forked(ARMATURE_OT_extrude={'forked': False}, TRANSFORM_OT_translate={'value': (0.5, 0, 1)})
bpy.data.armatures['Armature'].edit_bones['Bone_L']
bpy.data.armatures['Armature'].edit_bones['Bone_R']
bpy.data.objects['C0'].location = bpy.data.armatures['Armature'].edit_bones['Bone_L'].tail
bpy.data.objects['C0'].rotation_euler = bpy.data.armatures['Armature'].edit_bones['Bone_R'].tail
bpy.data.objects['C0'].scale.x = bpy.data.armatures['Armature'].edit_bones['Bone_R'].use_connect
bpy.ops.armature.extrude_forked()
bpy.data.armatures['Armature'].edit_bones['Bone_L.001']
bpy.data.armatures['Armature'].edit_bones['Bone_R.001']
bpy.data.armatures['Armature'].use_mirror_x = False
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.object.armature_add(location=(3, 0, 0), scale=(1, 1, 2))
bpy.data.objects['Armature'].select_set(True)
bpy.ops.object.mode_set(mode='EDIT')
bpy.context.objects_in_mode[1]
bpy.ops.armature.select_all(action='DESELECT')
bpy.data.armatures['Armature.001'].edit_bones['Bone'].select_tail = True
bpy.data.armatures['Armature'].edit_bones['Bone_L'].select_tail = True
bpy.ops.armature.select_more()
bpy.data.objects['C1'].location.y = bpy.data.armatures['Armature.001'].edit_bones['Bone'].select
bpy.data.objects['C1'].location.z = bpy.data.armatures['Armature'].edit_bones['Bone_L'].select
bpy.ops.armature.fill()
bpy.data.armatures['Armature.001'].edit_bones.active = bpy.data.armatures['Armature.001'].edit_bones['Bone']
bpy.ops.armature.select_similar(type='LENGTH', threshold=0.3)
bpy.data.objects['C1'].rotation_euler.x = bpy.data.armatures['Armature'].edit_bones['Bone'].select
bpy.data.objects['C1'].rotation_euler.y = bpy.data.armatures['Armature'].edit_bones['Bone_L.001'].select
bpy.ops.armature.align()
bpy.data.objects['C2'].location = bpy.data.armatures['Armature'].edit_bones['Bone_L'].tail
bpy.data.objects['C2'].rotation_euler = bpy.data.armatures['Armature'].edit_bones['Bone'].tail
bpy.ops.armature.hide()
bpy.data.objects['C1'].rotation_euler.z = bpy.data.armatures['Armature'].edit_bones['Bone'].hide
bpy.data.objects['C1'].scale.x = bpy.data.armatures['Armature.001'].edit_bones['Bone'].hide
bpy.ops.armature.reveal()
bpy.data.objects['C1'].scale.y = bpy.data.armatures['Armature'].edit_bones['Bone'].hide
bpy.ops.armature.flip_names()
bpy.data.armatures['Armature'].edit_bones['Bone_R']
bpy.ops.armature.split()
bpy.ops.armature.dissolve()
bpy.ops.armature.separate()
bpy.data.objects['Armature.002']
bpy.data.objects['Armature.003']
bpy.data.armatures['Armature'].edit_bones[4]
bpy.data.armatures['Armature'].edit_bones[5]
bpy.data.armatures['Armature.001'].edit_bones[0]
bpy.data.armatures['Armature.001'].edit_bones[1]
bpy.ops.object.mode_set(mode='OBJECT')
