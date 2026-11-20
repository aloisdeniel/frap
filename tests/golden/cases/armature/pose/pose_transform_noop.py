# WP 6.2, pose-mode transforms Blender finishes without a change or with proportional
# editing ignored: translating only connected (or fully location-locked) bones finishes and
# leaves every channel as it was; use_proportional_edit=True changes nothing (the pose
# TransData hold only the transformed bones).
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
bpy.ops.object.armature_add()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.armature.extrude_move(TRANSFORM_OT_translate={'value': (0, 0, 1)})
bpy.data.armatures['Armature'].edit_bones.new('Free')
bpy.data.armatures['Armature'].edit_bones['Free'].head = (1, 0, 0)
bpy.data.armatures['Armature'].edit_bones['Free'].tail = (1, 0, 1)
bpy.ops.object.mode_set(mode='POSE')
bpy.ops.pose.select_all(action='DESELECT')
bpy.data.objects['Armature'].pose.bones['Bone.001'].select = True
bpy.ops.transform.translate(value=(0.5, 0.3, 0.2))
bpy.ops.transform.translate(value=(0.5, 0.3, 0.2), orient_type='LOCAL')
bpy.data.objects['C0'].location = bpy.data.objects['Armature'].pose.bones['Bone.001'].location
bpy.data.objects['C0'].rotation_euler = bpy.data.objects['Armature'].pose.bones['Bone.001'].rotation_quaternion.to_euler()
bpy.data.objects['C0'].scale = bpy.data.objects['Armature'].pose.bones['Bone.001'].scale
bpy.data.objects['Armature'].pose.bones['Bone'].lock_location = (True, True, True)
bpy.data.objects['Armature'].pose.bones['Bone'].select = True
bpy.ops.transform.translate(value=(0.5, 0.3, 0.2))
bpy.data.objects['C1'].location = bpy.data.objects['Armature'].pose.bones['Bone'].location
bpy.data.objects['C1'].rotation_euler = bpy.data.objects['Armature'].pose.bones['Bone'].rotation_quaternion.to_euler()
bpy.ops.transform.resize(value=(2, 2, 2), use_proportional_edit=True)
bpy.data.objects['C1'].scale = bpy.data.objects['Armature'].pose.bones['Bone.001'].scale
bpy.ops.transform.rotate(value=0.3, orient_axis='Z', use_proportional_edit=True, proportional_size=0.5)
bpy.data.objects['C2'].rotation_euler = bpy.data.objects['Armature'].pose.bones['Bone.001'].rotation_quaternion.to_euler()
bpy.data.objects['C2'].scale = bpy.data.objects['Armature'].pose.bones['Bone'].scale
bpy.ops.pose.select_all(action='DESELECT')
bpy.data.objects['Armature'].pose.bones['Free'].select = True
bpy.ops.transform.translate(value=(1, 0, 0.5), use_proportional_edit=True, proportional_edit_falloff='SHARP', proportional_size=0.1)
bpy.data.objects['C3'].location = bpy.data.objects['Armature'].pose.bones['Free'].location
bpy.data.objects['C3'].scale = bpy.data.objects['Armature'].pose.bones['Bone'].location
