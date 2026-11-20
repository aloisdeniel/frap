# WP 6.2, the pose while the armature is in Edit Mode: Blender's pose evaluation returns
# early while edit bones exist, so PoseBone.matrix / head / tail keep the pose of the stored
# bones and the channels (not the rest pose) until Edit Mode is left; leaving re-evaluates
# with the new rest bones. A parent scaled to zero makes the child's matrix setter invert a
# singular matrix (invert_m4_m4 gives the zero matrix).
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
bpy.ops.object.armature_add(location=(0.5, 0, 0), rotation=(0, 0, 0.3))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.armature.extrude_move(TRANSFORM_OT_translate={'value': (0, 0.5, 1)})
bpy.ops.object.mode_set(mode='POSE')
bpy.data.objects['Armature'].pose.bones['Bone'].rotation_quaternion = (0.9, 0.4, 0, 0)
bpy.data.objects['Armature'].pose.bones['Bone.001'].rotation_mode = 'XYZ'
bpy.data.objects['Armature'].pose.bones['Bone.001'].rotation_euler = (0, 0.5, 0.3)
bpy.data.objects['Armature'].pose.bones['Bone.001'].scale = (1, 2, 1)
bpy.ops.object.mode_set(mode='EDIT')
bpy.context.view_layer.update()
bpy.data.objects['C0'].location = bpy.data.objects['Armature'].pose.bones['Bone'].head
bpy.data.objects['C0'].rotation_euler = bpy.data.objects['Armature'].pose.bones['Bone'].matrix.to_euler()
bpy.data.objects['C0'].scale = bpy.data.objects['Armature'].pose.bones['Bone'].tail
bpy.data.objects['C1'].location = bpy.data.objects['Armature'].pose.bones['Bone.001'].tail
bpy.data.objects['C1'].rotation_euler = bpy.data.objects['Armature'].pose.bones['Bone.001'].matrix.to_euler()
bpy.data.objects['C1'].scale = bpy.data.objects['Armature'].pose.bones['Bone.001'].matrix.to_scale()
bpy.data.armatures['Armature'].edit_bones['Bone'].tail = (0, 1, 1.5)
bpy.context.view_layer.update()
bpy.data.objects['C2'].location = bpy.data.objects['Armature'].pose.bones['Bone.001'].tail
bpy.data.objects['C2'].scale = bpy.data.objects['Armature'].pose.bones['Bone'].tail
bpy.ops.object.mode_set(mode='POSE')
bpy.context.view_layer.update()
bpy.data.objects['C3'].location = bpy.data.objects['Armature'].pose.bones['Bone.001'].tail
bpy.data.objects['C3'].scale = bpy.data.objects['Armature'].pose.bones['Bone'].tail
bpy.data.objects['Armature'].pose.bones['Bone'].scale = (0, 0, 0)
bpy.data.objects['Armature'].pose.bones['Bone.001'].rotation_euler = (0, 0, 0)
bpy.data.objects['Armature'].pose.bones['Bone.001'].scale = (1, 1, 1)
bpy.context.view_layer.update()
bpy.data.objects['Armature'].pose.bones['Bone.001'].matrix = ((bpy.data.objects['Armature'].pose.bones['Bone.001'].matrix.copy()))
bpy.data.objects['C4'].location = bpy.data.objects['Armature'].pose.bones['Bone.001'].location
bpy.data.objects['C4'].scale = bpy.data.objects['Armature'].pose.bones['Bone.001'].scale
bpy.data.objects['C5'].rotation_euler = bpy.data.objects['Armature'].pose.bones['Bone.001'].rotation_euler
