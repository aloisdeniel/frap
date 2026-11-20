# M5+M6 stabilization: the Python-only bone helpers of `_bpy_types.py` (`_GenericBone`) on
# EditBone, Bone and PoseBone: vector, center, x/y/z_axis, basename, parent_recursive,
# children (EditBone, PoseBone), children_recursive (sorted by depth),
# children_recursive_basename, translate() (edit bones only: Bone.head and PoseBone.head are
# read-only) and parent_index(); PoseBone.id_data / Bone.id_data. Results are moved into
# object locations, and names into new objects' names, so that they are compared.
# golden: startup=factory compare=objects,context tier=T0
import bpy
from mathutils import Vector
bpy.data.objects.new('C0', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['C0'])
bpy.data.objects.new('C1', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['C1'])
bpy.data.objects.new('C2', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['C2'])
bpy.data.objects.new('C3', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['C3'])
bpy.ops.object.armature_add(location=(1, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.data.armatures['Armature'].edit_bones['Bone'].name = 'arm.L'
bpy.data.armatures['Armature'].edit_bones.new('arm.L.001')
bpy.data.armatures['Armature'].edit_bones['arm.L.001'].head = (0, 0, 1)
bpy.data.armatures['Armature'].edit_bones['arm.L.001'].tail = (0, 1, 2)
bpy.data.armatures['Armature'].edit_bones['arm.L.001'].roll = 0.5
bpy.data.armatures['Armature'].edit_bones['arm.L.001'].parent = bpy.data.armatures['Armature'].edit_bones['arm.L']
bpy.data.armatures['Armature'].edit_bones.new('arm.L.002')
bpy.data.armatures['Armature'].edit_bones['arm.L.002'].head = (0, 1, 2)
bpy.data.armatures['Armature'].edit_bones['arm.L.002'].tail = (1, 1, 3)
bpy.data.armatures['Armature'].edit_bones['arm.L.002'].parent = bpy.data.armatures['Armature'].edit_bones['arm.L.001']
bpy.data.armatures['Armature'].edit_bones['arm.L.002'].use_connect = True
bpy.data.armatures['Armature'].edit_bones.new('hand')
bpy.data.armatures['Armature'].edit_bones['hand'].head = (1, 1, 3)
bpy.data.armatures['Armature'].edit_bones['hand'].tail = (1, 2, 3)
bpy.data.armatures['Armature'].edit_bones['hand'].parent = bpy.data.armatures['Armature'].edit_bones['arm.L.002']
bpy.data.armatures['Armature'].edit_bones.new('finger')
bpy.data.armatures['Armature'].edit_bones['finger'].head = (0, 0, 1)
bpy.data.armatures['Armature'].edit_bones['finger'].tail = (0, 0, 0)
bpy.data.armatures['Armature'].edit_bones['finger'].parent = bpy.data.armatures['Armature'].edit_bones['arm.L']
bpy.data.objects['C0'].location = bpy.data.armatures['Armature'].edit_bones['arm.L.002'].vector
bpy.data.objects['C1'].location = bpy.data.armatures['Armature'].edit_bones['arm.L.001'].center
bpy.data.objects['C2'].location = bpy.data.armatures['Armature'].edit_bones['arm.L.001'].x_axis
bpy.data.objects['C3'].location = bpy.data.armatures['Armature'].edit_bones['arm.L.001'].z_axis
bpy.data.objects.new(bpy.data.armatures['Armature'].edit_bones['arm.L.002'].basename, None)
bpy.data.objects.new(bpy.data.armatures['Armature'].edit_bones['hand'].parent_recursive[2].name, None)
bpy.data.objects.new(bpy.data.armatures['Armature'].edit_bones['arm.L'].children[1].name, None)
bpy.data.objects.new(bpy.data.armatures['Armature'].edit_bones['arm.L'].children_recursive[2].name, None)
bpy.data.objects['C0'].rotation_euler.x = bpy.data.armatures['Armature'].edit_bones['hand'].parent_index(bpy.data.armatures['Armature'].edit_bones['arm.L'])
bpy.data.objects['C0'].rotation_euler.y = bpy.data.armatures['Armature'].edit_bones['arm.L'].parent_index(bpy.data.armatures['Armature'].edit_bones['hand'])
bpy.data.objects.new(bpy.data.armatures['Armature'].edit_bones['arm.L'].children_recursive_basename[1].name, None)
bpy.data.armatures['Armature'].edit_bones['hand'].translate(Vector((0, 0, 0.5)))
bpy.data.armatures['Armature'].edit_bones['arm.L.001'].translate(Vector((0.25, 0, 0)))
bpy.data.objects['C1'].location = bpy.data.armatures['Armature'].edit_bones['arm.L.002'].head
bpy.data.objects['C1'].scale = bpy.data.armatures['Armature'].edit_bones['hand'].tail
bpy.ops.object.mode_set(mode='POSE')
bpy.data.objects['Armature'].pose.bones['arm.L.001'].rotation_mode = 'XYZ'
bpy.data.objects['Armature'].pose.bones['arm.L.001'].rotation_euler = (0.3, 0.2, 0.1)
bpy.context.view_layer.update()
bpy.data.objects['C2'].location = bpy.data.objects['Armature'].pose.bones['hand'].vector
bpy.data.objects['C2'].scale = bpy.data.objects['Armature'].pose.bones['hand'].center
bpy.data.objects['C2'].rotation_euler = bpy.data.objects['Armature'].pose.bones['hand'].y_axis
bpy.data.objects['C3'].location = bpy.data.armatures['Armature'].bones['hand'].vector
bpy.data.objects['C3'].scale = bpy.data.armatures['Armature'].bones['arm.L.001'].x_axis
bpy.data.objects['C3'].rotation_euler = bpy.data.armatures['Armature'].bones['arm.L.002'].center
bpy.data.objects.new(bpy.data.objects['Armature'].pose.bones['arm.L'].children[0].name, None)
bpy.data.objects.new(bpy.data.objects['Armature'].pose.bones['finger'].parent_recursive[0].name, None)
bpy.data.objects.new(bpy.data.objects['Armature'].pose.bones['arm.L'].children_recursive[3].name, None)
bpy.data.objects.new(bpy.data.armatures['Armature'].bones['arm.L'].children_recursive[1].name, None)
bpy.data.objects['C0'].rotation_euler.z = bpy.data.objects['Armature'].pose.bones['hand'].parent_index(bpy.data.armatures['Armature'].bones['arm.L.001'])
bpy.data.objects['C1'].rotation_euler.z = bpy.data.armatures['Armature'].bones['hand'].parent_index(bpy.data.armatures['Armature'].bones['hand'])
bpy.data.armatures['Armature'].bones['hand'].translate(Vector((1, 0, 0)))
bpy.data.objects['Armature'].pose.bones['hand'].translate(Vector((1, 0, 0)))
bpy.data.objects['Armature'].pose.bones['hand'].parent_index(None)
bpy.data.objects.new(bpy.data.objects['Armature'].pose.bones['hand'].id_data.name, None)
bpy.data.objects.new(bpy.data.armatures['Armature'].bones['hand'].id_data.name, None)
bpy.data.objects.new(bpy.data.objects['Armature'].pose.id_data.name, None)
bpy.data.objects.new(bpy.context.scene.render.id_data.name, None)
bpy.ops.object.mode_set(mode='EDIT')
bpy.data.armatures['Armature'].edit_bones['arm.L.001'].align_roll(Vector((1, 0, 0)))
bpy.data.objects['C0'].rotation_euler.x = bpy.data.armatures['Armature'].edit_bones['arm.L.001'].roll
bpy.data.armatures['Armature'].edit_bones['hand'].align_roll(Vector((0, 0.6, 0.8)))
bpy.data.objects['C0'].rotation_euler.y = bpy.data.armatures['Armature'].edit_bones['hand'].roll
bpy.data.armatures['Armature'].edit_bones['finger'].align_roll(Vector((0, 0, 1)))
bpy.data.objects['C0'].rotation_euler.z = bpy.data.armatures['Armature'].edit_bones['finger'].roll
