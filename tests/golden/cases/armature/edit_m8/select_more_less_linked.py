# WP 8.3.10, armature.select_more / select_less / select_linked(all_forks) /
# select_hierarchy(direction, extend) on a connected chain with a fork and a detached child:
# selection spreads and shrinks one bone along connected joints, linked selection walks the
# connected parents (and their forks with all_forks), hierarchy moves the active bone.
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
bpy.data.objects.new('N0', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N0'])
bpy.data.objects.new('N1', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N1'])
bpy.data.objects.new('N2', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N2'])
bpy.data.objects.new('N3', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N3'])
bpy.ops.object.armature_add()
bpy.ops.object.mode_set(mode='EDIT')
bpy.data.armatures['Armature'].edit_bones.new('B1')
bpy.data.armatures['Armature'].edit_bones['B1'].head = (0, 0, 1)
bpy.data.armatures['Armature'].edit_bones['B1'].tail = (0, 0, 2)
bpy.data.armatures['Armature'].edit_bones['B1'].parent = bpy.data.armatures['Armature'].edit_bones['Bone']
bpy.data.armatures['Armature'].edit_bones['B1'].use_connect = True
bpy.data.armatures['Armature'].edit_bones.new('B2')
bpy.data.armatures['Armature'].edit_bones['B2'].head = (0, 0, 2)
bpy.data.armatures['Armature'].edit_bones['B2'].tail = (0, 0, 3)
bpy.data.armatures['Armature'].edit_bones['B2'].parent = bpy.data.armatures['Armature'].edit_bones['B1']
bpy.data.armatures['Armature'].edit_bones['B2'].use_connect = True
bpy.data.armatures['Armature'].edit_bones.new('F')
bpy.data.armatures['Armature'].edit_bones['F'].head = (0, 0, 1)
bpy.data.armatures['Armature'].edit_bones['F'].tail = (1, 0, 2)
bpy.data.armatures['Armature'].edit_bones['F'].parent = bpy.data.armatures['Armature'].edit_bones['Bone']
bpy.data.armatures['Armature'].edit_bones['F'].use_connect = True
bpy.data.armatures['Armature'].edit_bones.new('D')
bpy.data.armatures['Armature'].edit_bones['D'].head = (2, 0, 0)
bpy.data.armatures['Armature'].edit_bones['D'].tail = (2, 0, 1)
bpy.data.armatures['Armature'].edit_bones['D'].parent = bpy.data.armatures['Armature'].edit_bones['B2']
bpy.ops.armature.select_all(action='DESELECT')
bpy.data.armatures['Armature'].edit_bones['B1'].select = True
bpy.data.armatures['Armature'].edit_bones['B1'].select_head = True
bpy.data.armatures['Armature'].edit_bones['B1'].select_tail = True
bpy.ops.armature.select_more()
bpy.data.objects['C0'].location.x = bpy.data.armatures['Armature'].edit_bones['Bone'].select
bpy.data.objects['C0'].location.y = bpy.data.armatures['Armature'].edit_bones['B2'].select
bpy.data.objects['C0'].location.z = bpy.data.armatures['Armature'].edit_bones['F'].select
bpy.data.objects['C0'].rotation_euler.x = bpy.data.armatures['Armature'].edit_bones['D'].select
bpy.data.objects['C0'].rotation_euler.y = bpy.data.armatures['Armature'].edit_bones['Bone'].select_head
bpy.data.objects['C0'].rotation_euler.z = bpy.data.armatures['Armature'].edit_bones['B2'].select_tail
bpy.ops.armature.select_more()
bpy.data.objects['C0'].scale.x = bpy.data.armatures['Armature'].edit_bones['D'].select
bpy.data.objects['C0'].scale.y = bpy.data.armatures['Armature'].edit_bones['F'].select_tail
bpy.ops.armature.select_less()
bpy.data.objects['C1'].location.x = bpy.data.armatures['Armature'].edit_bones['Bone'].select
bpy.data.objects['C1'].location.y = bpy.data.armatures['Armature'].edit_bones['B1'].select
bpy.data.objects['C1'].location.z = bpy.data.armatures['Armature'].edit_bones['B2'].select
bpy.data.objects['C1'].rotation_euler.x = bpy.data.armatures['Armature'].edit_bones['F'].select
bpy.data.objects['C1'].rotation_euler.y = bpy.data.armatures['Armature'].edit_bones['B2'].select_tail
bpy.data.objects['C1'].rotation_euler.z = bpy.data.armatures['Armature'].edit_bones['Bone'].select_head
bpy.data.armatures['Armature'].edit_bones['Bone'].select_head = False
bpy.ops.armature.select_less()
bpy.data.objects['C1'].scale.x = bpy.data.armatures['Armature'].edit_bones['Bone'].select
bpy.data.objects['C1'].scale.y = bpy.data.armatures['Armature'].edit_bones['B1'].select
bpy.data.objects['C1'].scale.z = bpy.data.armatures['Armature'].edit_bones['F'].select
bpy.ops.armature.select_all(action='DESELECT')
bpy.data.armatures['Armature'].edit_bones['B2'].select_tail = True
bpy.ops.armature.select_linked()
bpy.data.objects['C2'].location.x = bpy.data.armatures['Armature'].edit_bones['Bone'].select
bpy.data.objects['C2'].location.y = bpy.data.armatures['Armature'].edit_bones['B1'].select
bpy.data.objects['C2'].location.z = bpy.data.armatures['Armature'].edit_bones['F'].select
bpy.data.objects['C2'].rotation_euler.x = bpy.data.armatures['Armature'].edit_bones['D'].select
bpy.data.objects['C2'].rotation_euler.y = bpy.data.armatures['Armature'].edit_bones['B2'].select
bpy.ops.armature.select_all(action='DESELECT')
bpy.data.armatures['Armature'].edit_bones['B2'].select_tail = True
bpy.ops.armature.select_linked(all_forks=True)
bpy.data.objects['C2'].scale.x = bpy.data.armatures['Armature'].edit_bones['F'].select
bpy.data.objects['C2'].scale.y = bpy.data.armatures['Armature'].edit_bones['D'].select
bpy.ops.armature.select_all(action='DESELECT')
bpy.data.armatures['Armature'].edit_bones['F'].select = True
bpy.ops.armature.select_linked()
bpy.data.objects['C3'].location.x = bpy.data.armatures['Armature'].edit_bones['B1'].select
bpy.data.objects['C3'].location.y = bpy.data.armatures['Armature'].edit_bones['Bone'].select
bpy.data.objects['C3'].location.z = bpy.data.armatures['Armature'].edit_bones['B2'].select
bpy.ops.armature.select_all(action='DESELECT')
bpy.data.armatures['Armature'].edit_bones['D'].select = True
bpy.data.armatures['Armature'].edit_bones['D'].select_head = True
bpy.data.armatures['Armature'].edit_bones['D'].select_tail = True
bpy.ops.armature.select_linked()
bpy.data.objects['C3'].rotation_euler.x = bpy.data.armatures['Armature'].edit_bones['B2'].select
bpy.data.objects['C3'].rotation_euler.y = bpy.data.armatures['Armature'].edit_bones['D'].select
bpy.ops.armature.select_all(action='DESELECT')
bpy.data.armatures['Armature'].edit_bones.active = bpy.data.armatures['Armature'].edit_bones['B1']
bpy.data.armatures['Armature'].edit_bones['B1'].select = True
bpy.data.armatures['Armature'].edit_bones['B1'].select_head = True
bpy.data.armatures['Armature'].edit_bones['B1'].select_tail = True
bpy.ops.armature.select_hierarchy(direction='CHILD')
bpy.data.objects['N0'].name = bpy.context.active_bone.name
bpy.data.objects['C4'].location.x = bpy.data.armatures['Armature'].edit_bones['B1'].select
bpy.data.objects['C4'].location.y = bpy.data.armatures['Armature'].edit_bones['B2'].select
bpy.ops.armature.select_hierarchy(direction='CHILD', extend=True)
bpy.data.objects['N1'].name = bpy.context.active_bone.name
bpy.data.objects['C4'].location.z = bpy.data.armatures['Armature'].edit_bones['B2'].select
bpy.data.objects['C4'].rotation_euler.x = bpy.data.armatures['Armature'].edit_bones['D'].select
bpy.ops.armature.select_hierarchy(direction='CHILD')
bpy.ops.armature.select_hierarchy(direction='PARENT')
bpy.ops.armature.select_hierarchy(direction='PARENT', extend=True)
bpy.data.objects['N2'].name = bpy.context.active_bone.name
bpy.data.objects['C4'].rotation_euler.y = bpy.data.armatures['Armature'].edit_bones['B1'].select
bpy.data.objects['C4'].rotation_euler.z = bpy.data.armatures['Armature'].edit_bones['B2'].select
bpy.data.objects['C4'].scale.x = bpy.data.armatures['Armature'].edit_bones['D'].select
bpy.data.armatures['Armature'].edit_bones['Bone'].hide = True
bpy.ops.armature.select_hierarchy(direction='PARENT')
bpy.data.armatures['Armature'].edit_bones.active = bpy.data.armatures['Armature'].edit_bones['Bone']
bpy.data.armatures['Armature'].edit_bones['Bone'].hide = False
bpy.data.armatures['Armature'].edit_bones['B1'].hide_select = True
bpy.ops.armature.select_hierarchy(direction='CHILD')
bpy.data.objects['N3'].name = bpy.context.active_bone.name
bpy.ops.armature.select_all(action='SELECT')
bpy.ops.armature.select_less()
bpy.data.objects['C5'].location.x = bpy.data.armatures['Armature'].edit_bones['B1'].select
bpy.data.objects['C5'].location.y = bpy.data.armatures['Armature'].edit_bones['B2'].select
bpy.data.objects['C5'].location.z = bpy.data.armatures['Armature'].edit_bones['F'].select
bpy.ops.object.mode_set(mode='OBJECT')
