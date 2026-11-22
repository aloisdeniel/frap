# WP 8.3.10, armature.hide(unselected) / reveal(select) with unselectable bones and hidden bone
# collections; armature.dissolve merging selected connected chains (tails, radii and tail
# selection taken from the dissolved child, grandchildren moved up, pose channels removed,
# use_mirror_x dissolving the mirror chain, nothing to dissolve cancels); armature.split
# disconnecting the selection from its unselected neighbours.
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
bpy.data.objects.new('N0', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N0'])
bpy.ops.object.armature_add()
bpy.ops.object.mode_set(mode='EDIT')
bpy.data.armatures['Armature'].edit_bones.new('a.L')
bpy.data.armatures['Armature'].edit_bones['a.L'].head = (0.5, 0, 0)
bpy.data.armatures['Armature'].edit_bones['a.L'].tail = (1, 0, 0.5)
bpy.data.armatures['Armature'].edit_bones['a.L'].tail_radius = 0.2
bpy.data.armatures['Armature'].edit_bones.new('b.L')
bpy.data.armatures['Armature'].edit_bones['b.L'].head = (1, 0, 0.5)
bpy.data.armatures['Armature'].edit_bones['b.L'].tail = (1.5, 0, 1)
bpy.data.armatures['Armature'].edit_bones['b.L'].tail_radius = 0.3
bpy.data.armatures['Armature'].edit_bones['b.L'].parent = bpy.data.armatures['Armature'].edit_bones['a.L']
bpy.data.armatures['Armature'].edit_bones['b.L'].use_connect = True
bpy.data.armatures['Armature'].edit_bones.new('c.L')
bpy.data.armatures['Armature'].edit_bones['c.L'].head = (1.5, 0, 1)
bpy.data.armatures['Armature'].edit_bones['c.L'].tail = (2, 0, 1)
bpy.data.armatures['Armature'].edit_bones['c.L'].tail_radius = 0.4
bpy.data.armatures['Armature'].edit_bones['c.L'].parent = bpy.data.armatures['Armature'].edit_bones['b.L']
bpy.data.armatures['Armature'].edit_bones['c.L'].use_connect = True
bpy.data.armatures['Armature'].edit_bones.new('d.L')
bpy.data.armatures['Armature'].edit_bones['d.L'].head = (2, 0, 1)
bpy.data.armatures['Armature'].edit_bones['d.L'].tail = (2.5, 0, 1)
bpy.data.armatures['Armature'].edit_bones['d.L'].parent = bpy.data.armatures['Armature'].edit_bones['c.L']
bpy.data.armatures['Armature'].edit_bones['d.L'].use_connect = True
bpy.data.armatures['Armature'].edit_bones.new('a.R')
bpy.data.armatures['Armature'].edit_bones['a.R'].head = (-0.5, 0, 0)
bpy.data.armatures['Armature'].edit_bones['a.R'].tail = (-1, 0, 0.5)
bpy.data.armatures['Armature'].edit_bones.new('b.R')
bpy.data.armatures['Armature'].edit_bones['b.R'].head = (-1, 0, 0.5)
bpy.data.armatures['Armature'].edit_bones['b.R'].tail = (-1.5, 0, 1)
bpy.data.armatures['Armature'].edit_bones['b.R'].tail_radius = 0.25
bpy.data.armatures['Armature'].edit_bones['b.R'].parent = bpy.data.armatures['Armature'].edit_bones['a.R']
bpy.data.armatures['Armature'].edit_bones['b.R'].use_connect = True
bpy.data.armatures['Armature'].edit_bones.new('fork')
bpy.data.armatures['Armature'].edit_bones['fork'].head = (1, 0, 0.5)
bpy.data.armatures['Armature'].edit_bones['fork'].tail = (1, 1, 0.5)
bpy.data.armatures['Armature'].edit_bones['fork'].parent = bpy.data.armatures['Armature'].edit_bones['a.L']
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.armature.select_all(action='DESELECT')
bpy.ops.armature.hide()
bpy.data.armatures['Armature'].edit_bones['fork'].select = True
bpy.data.armatures['Armature'].edit_bones['fork'].select_head = True
bpy.data.armatures['Armature'].edit_bones['fork'].select_tail = True
bpy.ops.armature.hide()
bpy.data.objects['C0'].location.x = bpy.data.armatures['Armature'].edit_bones['fork'].hide
bpy.data.objects['C0'].location.y = bpy.data.armatures['Armature'].edit_bones['fork'].select
bpy.data.armatures['Armature'].edit_bones['Bone'].select = True
bpy.data.armatures['Armature'].edit_bones['Bone'].select_head = True
bpy.data.armatures['Armature'].edit_bones['Bone'].select_tail = True
bpy.ops.armature.hide(unselected=True)
bpy.data.objects['C0'].location.z = bpy.data.armatures['Armature'].edit_bones['a.L'].hide
bpy.data.objects['C0'].rotation_euler.x = bpy.data.armatures['Armature'].edit_bones['Bone'].hide
bpy.data.armatures['Armature'].edit_bones['a.R'].hide_select = True
bpy.ops.armature.reveal()
bpy.data.objects['C0'].rotation_euler.y = bpy.data.armatures['Armature'].edit_bones['a.L'].select
bpy.data.objects['C0'].rotation_euler.z = bpy.data.armatures['Armature'].edit_bones['a.R'].select
bpy.data.objects['C0'].scale.x = bpy.data.armatures['Armature'].edit_bones['a.R'].hide
bpy.data.objects['C0'].scale.y = bpy.data.armatures['Armature'].edit_bones['fork'].select_head
bpy.data.armatures['Armature'].edit_bones['a.R'].hide_select = False
bpy.ops.armature.select_all(action='DESELECT')
bpy.data.armatures['Armature'].edit_bones['d.L'].select = True
bpy.data.armatures['Armature'].edit_bones['d.L'].select_head = True
bpy.data.armatures['Armature'].edit_bones['d.L'].select_tail = True
bpy.ops.armature.hide()
bpy.ops.armature.reveal(select=False)
bpy.data.objects['C0'].scale.z = bpy.data.armatures['Armature'].edit_bones['d.L'].select
bpy.ops.armature.select_all(action='DESELECT')
bpy.ops.armature.dissolve()
bpy.data.armatures['Armature'].use_mirror_x = True
bpy.data.armatures['Armature'].edit_bones['a.L'].select = True
bpy.data.armatures['Armature'].edit_bones['a.L'].select_head = True
bpy.data.armatures['Armature'].edit_bones['a.L'].select_tail = True
bpy.ops.armature.dissolve()
bpy.data.armatures['Armature'].edit_bones['b.L']
bpy.data.armatures['Armature'].edit_bones['b.R']
bpy.data.objects['C1'].location = bpy.data.armatures['Armature'].edit_bones['a.L'].tail
bpy.data.objects['C1'].rotation_euler.x = bpy.data.armatures['Armature'].edit_bones['a.L'].tail_radius
bpy.data.objects['C1'].rotation_euler.y = bpy.data.armatures['Armature'].edit_bones['a.L'].select_tail
bpy.data.objects['C1'].rotation_euler.z = bpy.data.armatures['Armature'].edit_bones['c.L'].head_radius
bpy.data.objects['N0'].name = bpy.data.armatures['Armature'].edit_bones['c.L'].parent.name
bpy.data.objects['C1'].scale.x = bpy.data.armatures['Armature'].edit_bones['c.L'].use_connect
bpy.data.objects['C1'].scale.y = bpy.data.armatures['Armature'].edit_bones['fork'].use_connect
bpy.data.objects['C1'].scale.z = bpy.data.armatures['Armature'].edit_bones['a.R'].select
bpy.data.objects['C2'].location = bpy.data.armatures['Armature'].edit_bones['a.R'].tail
bpy.data.objects['C2'].rotation_euler.x = bpy.data.armatures['Armature'].edit_bones['a.R'].tail_radius
bpy.data.objects['C2'].rotation_euler.y = bpy.data.armatures['Armature'].edit_bones['a.R'].select_tail
bpy.data.objects['C2'].rotation_euler.z = bpy.data.armatures['Armature'].edit_bones['a.L'].select
bpy.data.armatures['Armature'].use_mirror_x = False
bpy.ops.armature.select_all(action='DESELECT')
bpy.data.armatures['Armature'].edit_bones['a.L'].select = True
bpy.data.armatures['Armature'].edit_bones['a.L'].select_head = True
bpy.data.armatures['Armature'].edit_bones['a.L'].select_tail = True
bpy.data.armatures['Armature'].edit_bones['c.L'].select = True
bpy.data.armatures['Armature'].edit_bones['c.L'].select_tail = True
bpy.data.armatures['Armature'].edit_bones['d.L'].select = True
bpy.data.armatures['Armature'].edit_bones['d.L'].select_tail = True
bpy.ops.armature.dissolve()
bpy.data.armatures['Armature'].edit_bones['c.L']
bpy.data.armatures['Armature'].edit_bones['d.L']
bpy.data.objects['C2'].scale = bpy.data.armatures['Armature'].edit_bones['a.L'].tail
bpy.data.objects['C4'].location.x = bpy.data.armatures['Armature'].edit_bones['a.L'].tail_radius
bpy.data.objects['C4'].location.y = bpy.data.armatures['Armature'].edit_bones['a.L'].select
bpy.data.armatures['Armature'].use_mirror_x = False
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Armature'].pose.bones['b.L']
bpy.data.objects['Armature'].pose.bones['b.R']
bpy.data.objects['Armature'].pose.bones['c.L']
bpy.data.objects['Armature'].pose.bones['a.R']
bpy.ops.object.mode_set(mode='EDIT')
bpy.data.armatures['Armature'].edit_bones.new('e')
bpy.data.armatures['Armature'].edit_bones['e'].head = (1, 1, 0.5)
bpy.data.armatures['Armature'].edit_bones['e'].tail = (1, 2, 0.5)
bpy.data.armatures['Armature'].edit_bones['e'].parent = bpy.data.armatures['Armature'].edit_bones['fork']
bpy.data.armatures['Armature'].edit_bones['e'].use_connect = True
bpy.data.armatures['Armature'].edit_bones.new('f')
bpy.data.armatures['Armature'].edit_bones['f'].head = (1, 2, 0.5)
bpy.data.armatures['Armature'].edit_bones['f'].tail = (1, 3, 0.5)
bpy.data.armatures['Armature'].edit_bones['f'].parent = bpy.data.armatures['Armature'].edit_bones['e']
bpy.data.armatures['Armature'].edit_bones['f'].use_connect = True
bpy.ops.armature.select_all(action='DESELECT')
bpy.data.armatures['Armature'].edit_bones['e'].select = True
bpy.data.armatures['Armature'].edit_bones['e'].select_head = True
bpy.data.armatures['Armature'].edit_bones['e'].select_tail = True
bpy.ops.armature.split()
bpy.data.objects['C3'].location.x = bpy.data.armatures['Armature'].edit_bones['e'].use_connect
bpy.data.objects['C3'].location.y = bpy.data.armatures['Armature'].edit_bones['f'].use_connect
bpy.data.armatures['Armature'].edit_bones['e'].parent
bpy.data.armatures['Armature'].edit_bones['f'].parent
bpy.data.objects['C3'].location.z = bpy.data.armatures['Armature'].edit_bones['fork'].select_tail
bpy.data.objects['C3'].rotation_euler.x = bpy.data.armatures['Armature'].edit_bones['f'].select_head
bpy.data.objects['C3'].rotation_euler.y = bpy.data.armatures['Armature'].edit_bones['e'].select
bpy.data.objects['C3'].rotation_euler.z = bpy.data.armatures['Armature'].edit_bones['fork'].select
bpy.data.armatures['Armature'].edit_bones['f'].parent = bpy.data.armatures['Armature'].edit_bones['e']
bpy.data.armatures['Armature'].edit_bones['f'].use_connect = True
bpy.ops.armature.select_all(action='SELECT')
bpy.ops.armature.split()
bpy.data.objects['C3'].scale.x = bpy.data.armatures['Armature'].edit_bones['f'].use_connect
bpy.data.objects['C3'].scale.y = bpy.data.armatures['Armature'].edit_bones['f'].select
bpy.ops.object.mode_set(mode='OBJECT')
