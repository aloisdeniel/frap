# WP 8.3.10, armature.separate: the selected bones move to a new armature object
# (Armature.001 with data Armature.001, in the original's collections, with its transform,
# base selection, copied action, bone collections and the moved bones' pose channels, in Object
# Mode); children of moved bones lose their parent; an object bone-parented to a moved bone
# follows it; the original keeps the rest, stays active in Edit Mode with nothing selected;
# only joints selected just deselects; nothing selected does nothing. Blender does not tag the
# re-parented child for evaluation (its matrix_world stays at the armature's until something
# else updates it), so the Blender side tags it before the dump; frap always evaluates fresh.
# golden: startup=factory compare=objects,context tier=T0 tol=1e-5
import bpy
bpy.data.objects.new('C0', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['C0'])
bpy.data.objects.new('C1', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['C1'])
bpy.data.objects.new('C2', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['C2'])
bpy.data.objects.new('N0', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N0'])
bpy.data.objects.new('N1', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N1'])
bpy.data.objects.new('N2', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N2'])
bpy.ops.object.armature_add(location=(1, 2, 0), rotation=(0.2, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.data.armatures['Armature'].edit_bones.new('B1')
bpy.data.armatures['Armature'].edit_bones['B1'].head = (0, 0, 1)
bpy.data.armatures['Armature'].edit_bones['B1'].tail = (0, 1, 1)
bpy.data.armatures['Armature'].edit_bones['B1'].parent = bpy.data.armatures['Armature'].edit_bones['Bone']
bpy.data.armatures['Armature'].edit_bones['B1'].use_connect = True
bpy.data.armatures['Armature'].edit_bones.new('B2')
bpy.data.armatures['Armature'].edit_bones['B2'].head = (0, 1, 1)
bpy.data.armatures['Armature'].edit_bones['B2'].tail = (0, 1, 2)
bpy.data.armatures['Armature'].edit_bones['B2'].parent = bpy.data.armatures['Armature'].edit_bones['B1']
bpy.data.armatures['Armature'].edit_bones['B2'].use_connect = True
bpy.data.armatures['Armature'].edit_bones.new('B3')
bpy.data.armatures['Armature'].edit_bones['B3'].head = (0, 1, 2)
bpy.data.armatures['Armature'].edit_bones['B3'].tail = (1, 1, 2)
bpy.data.armatures['Armature'].edit_bones['B3'].parent = bpy.data.armatures['Armature'].edit_bones['B2']
bpy.ops.object.mode_set(mode='POSE')
bpy.data.objects['Armature'].pose.bones['B1'].location = (0.1, 0.2, 0.3)
bpy.data.objects['Armature'].pose.bones['B2'].rotation_mode = 'XYZ'
bpy.data.objects['Armature'].pose.bones['B2'].rotation_euler = (0.4, 0, 0)
bpy.data.objects['Armature'].pose.bones['Bone'].scale = (1, 2, 1)
bpy.data.objects['Armature'].keyframe_insert('location')
bpy.ops.armature.collection_add()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['C2'].parent = bpy.data.objects['Armature']
bpy.data.objects['C2'].parent_type = 'BONE'
bpy.data.objects['C2'].parent_bone = 'B2'
bpy.data.objects['C1'].parent = bpy.data.objects['Armature']
bpy.data.objects['C1'].parent_type = 'BONE'
bpy.data.objects['C1'].parent_bone = 'Bone'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.armature.select_all(action='DESELECT')
bpy.ops.armature.separate()
bpy.data.objects['Armature.001']
bpy.data.armatures['Armature'].edit_bones['Bone'].select_tail = True
bpy.ops.armature.separate()
bpy.data.objects['Armature.001']
bpy.data.objects['C0'].location.x = bpy.data.armatures['Armature'].edit_bones['Bone'].select_tail
bpy.data.armatures['Armature'].edit_bones.active = bpy.data.armatures['Armature'].edit_bones['B2']
bpy.data.armatures['Armature'].edit_bones['B1'].select = True
bpy.data.armatures['Armature'].edit_bones['B1'].select_head = True
bpy.data.armatures['Armature'].edit_bones['B1'].select_tail = True
bpy.data.armatures['Armature'].edit_bones['B2'].select = True
bpy.data.armatures['Armature'].edit_bones['B2'].select_head = True
bpy.data.armatures['Armature'].edit_bones['B2'].select_tail = True
bpy.data.objects['Armature'].select_set(False)
bpy.ops.armature.separate()
bpy.data.objects['C0'].location.y = bpy.data.objects['Armature.001'].select_get()
bpy.data.objects['C0'].location.z = bpy.data.armatures['Armature'].edit_bones['Bone'].select_tail
bpy.data.armatures['Armature'].edit_bones['B1']
bpy.data.armatures['Armature'].edit_bones['B3']
bpy.data.armatures['Armature'].edit_bones['B3'].parent.name
bpy.data.objects['C0'].rotation_euler.x = bpy.data.armatures['Armature'].edit_bones['B3'].select
bpy.data.objects['N0'].name = bpy.data.objects['Armature.001'].data.name
bpy.data.objects['N1'].name = bpy.data.objects['Armature.001'].animation_data.action.name
bpy.data.objects['N2'].name = bpy.data.armatures['Armature.001'].bones.active.name
bpy.data.armatures['Armature.001'].bones['B1']
bpy.data.armatures['Armature.001'].bones['B2']
bpy.data.armatures['Armature.001'].bones['Bone']
bpy.data.armatures['Armature.001'].bones['B1'].parent.name
bpy.data.armatures['Armature.001'].bones['B2'].parent.name
bpy.data.armatures['Armature.001'].collections_all['Bones']
bpy.data.objects['C0'].rotation_euler.y = bpy.data.armatures['Armature.001'].collections.active_index
bpy.data.objects['C0'].scale = bpy.data.objects['Armature.001'].pose.bones['B1'].location
bpy.data.objects['C1'].location = bpy.data.objects['Armature.001'].pose.bones['B2'].rotation_euler
bpy.data.objects['Armature.001'].pose.bones['Bone']
bpy.data.objects['Armature'].pose.bones['B1']
bpy.data.objects['Armature'].pose.bones['B3']
bpy.data.objects['C1'].scale = bpy.data.objects['Armature'].pose.bones['Bone'].scale
bpy.data.objects['Armature'].select_set(True)
bpy.data.objects['Armature.001'].select_set(True)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Armature'].select_set(False)
bpy.data.objects['Armature.001'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Armature.001']
bpy.ops.object.mode_set(mode='EDIT')
bpy.data.armatures['Armature.001'].edit_bones['B2'].select = True
bpy.data.armatures['Armature.001'].edit_bones['B2'].select_head = True
bpy.data.armatures['Armature.001'].edit_bones['B2'].select_tail = True
bpy.ops.armature.separate()
bpy.data.objects['Armature.002']
bpy.data.armatures['Armature.002'].bones['B2']
bpy.data.armatures['Armature.002'].bones['B2'].parent
bpy.ops.object.mode_set(mode='OBJECT')
#@blender bpy.data.objects['C2'].update_tag()
