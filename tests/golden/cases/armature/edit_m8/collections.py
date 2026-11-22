# WP 8.3.10, bone collections on Armature.collections_all: collection_add (Bones, Bones.001,
# a sibling right after the active collection, in Object, Edit and Pose Mode),
# assign_to_collection (new_collection_name as a root or as the child of collection_index,
# existing index, out-of-range index error, Object Mode error leaving the new collection),
# move_to_collection (leaves every other collection), collection_assign / collection_unassign
# (named or active, unknown name error, nothing selected / nothing changed cancels, Pose Mode
# bones), collection_remove (children lifted into its place, next / previous sibling or parent
# made active, bones unassigned), and the polls. The Object Mode error of
# assign_to_collection comes last: Blender keeps the collection it created before failing,
# frap rolls the failed statement back (deviation D-150).
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
bpy.data.objects.new('N0', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N0'])
bpy.data.objects.new('N1', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N1'])
bpy.data.objects.new('N2', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N2'])
bpy.data.objects.new('N3', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N3'])
bpy.data.objects.new('N4', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N4'])
bpy.data.objects.new('N5', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N5'])
bpy.data.objects.new('N6', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N6'])
bpy.ops.armature.collection_add()
bpy.ops.armature.collection_assign()
bpy.ops.object.armature_add()
bpy.ops.armature.collection_remove()
bpy.ops.armature.collection_add()
bpy.ops.armature.collection_add()
bpy.ops.armature.collection_assign()
bpy.ops.armature.assign_to_collection(collection_index=0)
bpy.data.armatures['Armature'].collections_all['Bones']
bpy.data.armatures['Armature'].collections_all['Bones.001']
bpy.data.armatures['Armature'].collections_all['Bones.002']
bpy.ops.object.mode_set(mode='EDIT')
bpy.data.armatures['Armature'].edit_bones.new('B1')
bpy.data.armatures['Armature'].edit_bones['B1'].head = (0, 0, 1)
bpy.data.armatures['Armature'].edit_bones['B1'].tail = (0, 1, 1)
bpy.data.armatures['Armature'].edit_bones.new('B2')
bpy.data.armatures['Armature'].edit_bones['B2'].head = (0, 1, 1)
bpy.data.armatures['Armature'].edit_bones['B2'].tail = (0, 1, 2)
bpy.data.armatures['Armature'].edit_bones['B2'].lock = True
bpy.ops.armature.select_all(action='DESELECT')
bpy.ops.armature.collection_assign()
bpy.ops.armature.select_all(action='SELECT')
bpy.ops.armature.collection_assign(name='Nope')
bpy.ops.armature.collection_assign(name='Bones')
bpy.ops.armature.collection_assign(name='Bones')
bpy.data.armatures['Armature'].edit_bones['Bone'].collections['Bones']
bpy.data.armatures['Armature'].edit_bones['B2'].collections['Bones']
bpy.ops.armature.collection_assign()
bpy.data.armatures['Armature'].edit_bones['B1'].collections['Bones.001']
bpy.data.objects['C0'].location.x = bpy.data.armatures['Armature'].collections.active_index
bpy.ops.armature.assign_to_collection(collection_index=0, new_collection_name='Kid')
bpy.data.objects['N0'].name = bpy.data.armatures['Armature'].collections_all['Kid'].parent.name
bpy.data.objects['C0'].location.y = bpy.data.armatures['Armature'].collections_all['Kid'].index
bpy.data.objects['C0'].location.z = bpy.data.armatures['Armature'].collections.active_index
bpy.ops.armature.collection_add()
bpy.data.objects['N1'].name = bpy.data.armatures['Armature'].collections_all[4].name
bpy.data.objects['N2'].name = bpy.data.armatures['Armature'].collections_all[4].parent.name
bpy.data.objects['C0'].rotation_euler.x = bpy.data.armatures['Armature'].collections.active_index
bpy.ops.armature.assign_to_collection(new_collection_name='Root2')
bpy.data.objects['C0'].rotation_euler.y = bpy.data.armatures['Armature'].collections_all['Root2'].index
bpy.data.objects['C0'].rotation_euler.z = bpy.data.armatures['Armature'].collections_all['Kid'].index
bpy.ops.armature.assign_to_collection(collection_index=9)
bpy.ops.armature.assign_to_collection(collection_index=2)
bpy.ops.armature.assign_to_collection(collection_index=2)
bpy.ops.armature.move_to_collection(collection_index=1)
bpy.data.armatures['Armature'].edit_bones['Bone'].collections['Bones']
bpy.data.armatures['Armature'].edit_bones['Bone'].collections['Bones.001']
bpy.data.objects['C1'].location.x = bpy.data.armatures['Armature'].edit_bones['Bone'].collections[0].index
bpy.ops.armature.move_to_collection(new_collection_name='Moved', collection_index=1)
bpy.data.objects['N3'].name = bpy.data.armatures['Armature'].edit_bones['B1'].collections[0].name
bpy.data.objects['C1'].location.y = bpy.data.armatures['Armature'].collections_all['Moved'].index
bpy.data.armatures['Armature'].edit_bones['B1'].collections['Bones.001']
bpy.ops.armature.collection_unassign(name='Bones.001')
bpy.ops.armature.collection_unassign(name='Moved')
bpy.data.armatures['Armature'].edit_bones['B1'].collections['Moved']
bpy.ops.armature.select_all(action='DESELECT')
bpy.ops.armature.collection_unassign()
bpy.ops.object.mode_set(mode='POSE')
bpy.ops.pose.select_all(action='DESELECT')
bpy.data.objects['Armature'].pose.bones['B2'].select = True
bpy.data.armatures['Armature'].collections.active_name = 'Kid'
bpy.ops.armature.collection_assign()
bpy.ops.armature.collection_assign()
bpy.data.armatures['Armature'].bones['B2'].collections['Kid']
bpy.ops.armature.collection_unassign(name='Kid')
bpy.data.armatures['Armature'].bones['B2'].collections['Kid']
bpy.ops.armature.assign_to_collection(collection_index=0)
bpy.ops.armature.collection_add()
bpy.data.objects['N4'].name = bpy.data.armatures['Armature'].collections.active_name
bpy.data.objects['C1'].location.z = bpy.data.armatures['Armature'].collections.active_index
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.armature.collection_assign()
bpy.ops.armature.collection_add()
bpy.data.armatures['Armature'].collections.active.name = 'InObject'
bpy.data.armatures['Armature'].collections.active_name = 'Bones'
bpy.ops.armature.collection_remove()
bpy.data.objects['N5'].name = bpy.data.armatures['Armature'].collections.active_name
bpy.data.objects['C2'].location.x = bpy.data.armatures['Armature'].collections.active_index
bpy.data.objects['C2'].location.y = bpy.data.armatures['Armature'].collections_all['Kid'].index
bpy.data.armatures['Armature'].collections_all['Kid'].parent.name
bpy.data.armatures['Armature'].bones['B2'].collections['Bones']
bpy.data.armatures['Armature'].bones['Bone'].collections['Bones']
bpy.data.armatures['Armature'].collections.active_name = 'InObject'
bpy.ops.armature.collection_remove()
bpy.data.objects['N6'].name = bpy.data.armatures['Armature'].collections.active_name
bpy.data.objects['C2'].location.z = bpy.data.armatures['Armature'].collections.active_index
bpy.data.armatures['Armature'].collections.active_index = 2
bpy.ops.armature.collection_remove()
bpy.data.objects['C2'].rotation_euler.x = bpy.data.armatures['Armature'].collections.active_index
bpy.data.armatures['Armature'].collections.active_index = -1
bpy.ops.armature.collection_remove()
bpy.ops.object.mode_set(mode='EDIT')
bpy.data.armatures['Armature'].collections.active_index = 0
bpy.ops.armature.collection_remove()
bpy.data.objects['C3'].location.x = bpy.data.armatures['Armature'].collections.active_index
bpy.data.objects['C3'].location.y = bpy.data.armatures['Armature'].collections_all[0].index
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.armature.collection_assign()
bpy.ops.armature.move_to_collection()
bpy.ops.armature.collection_add()
bpy.context.view_layer.objects.active = bpy.data.objects['Armature']
bpy.ops.armature.assign_to_collection(new_collection_name='InObject2')
