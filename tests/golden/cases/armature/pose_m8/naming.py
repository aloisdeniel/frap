# WP 8.3.11, pose.flip_names (selected visible bones, .L/.R pairs swapping through the second
# pass, the animation paths following, do_strip_numbers) and pose.autoside_names (X, Y, Z
# from the rest bone's head and tail relative to its parent, as Blender's Pose Mode reads them;
# hidden bones untouched), and their Pose Mode polls. N<n> hold names.
# golden: startup=factory compare=objects,anim:ArmatureAction tier=T0 tol=1e-5
import bpy
bpy.ops.object.armature_add()
bpy.ops.object.mode_set(mode='EDIT')
bpy.data.armatures['Armature'].edit_bones.new('arm.L')
bpy.data.armatures['Armature'].edit_bones['arm.L'].head = (1, 0, 0.5)
bpy.data.armatures['Armature'].edit_bones['arm.L'].tail = (2, 0, 0.5)
bpy.data.armatures['Armature'].edit_bones['arm.L'].parent = bpy.data.armatures['Armature'].edit_bones['Bone']
bpy.data.armatures['Armature'].edit_bones.new('arm.R')
bpy.data.armatures['Armature'].edit_bones['arm.R'].head = (-1, 0, 0.5)
bpy.data.armatures['Armature'].edit_bones['arm.R'].tail = (-2, 0, 0.5)
bpy.data.armatures['Armature'].edit_bones['arm.R'].parent = bpy.data.armatures['Armature'].edit_bones['Bone']
bpy.data.armatures['Armature'].edit_bones.new('hand.L')
bpy.data.armatures['Armature'].edit_bones['hand.L'].head = (2, 0, 0.5)
bpy.data.armatures['Armature'].edit_bones['hand.L'].tail = (3, 0, 0.7)
bpy.data.armatures['Armature'].edit_bones['hand.L'].parent = bpy.data.armatures['Armature'].edit_bones['arm.L']
bpy.data.armatures['Armature'].edit_bones['hand.L'].use_connect = True
bpy.data.armatures['Armature'].edit_bones.new('hand.R')
bpy.data.armatures['Armature'].edit_bones['hand.R'].head = (-2, 0, 0.5)
bpy.data.armatures['Armature'].edit_bones['hand.R'].tail = (-3, 0, 0.7)
bpy.data.armatures['Armature'].edit_bones['hand.R'].parent = bpy.data.armatures['Armature'].edit_bones['arm.R']
bpy.data.armatures['Armature'].edit_bones['hand.R'].use_connect = True
bpy.data.armatures['Armature'].edit_bones.new('B1')
bpy.data.armatures['Armature'].edit_bones['B1'].head = (0, 0, 1)
bpy.data.armatures['Armature'].edit_bones['B1'].tail = (0, 0.3, 2)
bpy.data.armatures['Armature'].edit_bones['B1'].parent = bpy.data.armatures['Armature'].edit_bones['Bone']
bpy.data.armatures['Armature'].edit_bones['B1'].use_connect = True
bpy.data.armatures['Armature'].edit_bones.new('leg.L')
bpy.data.armatures['Armature'].edit_bones['leg.L'].head = (0.4, 0, 0)
bpy.data.armatures['Armature'].edit_bones['leg.L'].tail = (0.5, 0.1, -1)
bpy.data.armatures['Armature'].edit_bones.new('leg.R')
bpy.data.armatures['Armature'].edit_bones['leg.R'].head = (-0.4, 0, 0)
bpy.data.armatures['Armature'].edit_bones['leg.R'].tail = (-0.5, 0.1, -1)
bpy.data.armatures['Armature'].edit_bones.new('Bone.L.001')
bpy.data.armatures['Armature'].edit_bones['Bone.L.001'].head = (0.5, 0.5, 0.5)
bpy.data.armatures['Armature'].edit_bones['Bone.L.001'].tail = (0.5, 1.5, 0.5)
bpy.data.armatures['Armature'].edit_bones['Bone.L.001'].parent = bpy.data.armatures['Armature'].edit_bones['arm.L']
bpy.data.armatures['Armature'].edit_bones.new('tip')
bpy.data.armatures['Armature'].edit_bones['tip'].head = (0.2, -0.3, 1.5)
bpy.data.armatures['Armature'].edit_bones['tip'].tail = (0.4, -0.3, 2)
bpy.data.armatures['Armature'].edit_bones['tip'].parent = bpy.data.armatures['Armature'].edit_bones['B1']
bpy.data.armatures['Armature'].edit_bones.new('mid')
bpy.data.armatures['Armature'].edit_bones['mid'].head = (-0.3, 0.2, 1.5)
bpy.data.armatures['Armature'].edit_bones['mid'].tail = (-0.3, 0.2, 2.5)
bpy.data.armatures['Armature'].edit_bones['mid'].parent = bpy.data.armatures['Armature'].edit_bones['B1']
bpy.ops.object.mode_set(mode='POSE')
bpy.data.objects['Armature'].pose.bones['arm.L'].keyframe_insert('location', frame=1)
bpy.ops.pose.select_all(action='DESELECT')
bpy.data.objects['Armature'].pose.bones['arm.L'].select = True
bpy.data.objects['Armature'].pose.bones['hand.L'].select = True
bpy.data.objects['Armature'].pose.bones['leg.R'].select = True
bpy.data.objects['Armature'].pose.bones['B1'].select = True
bpy.ops.pose.flip_names()
bpy.data.objects.new('N0', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N0'])
bpy.data.objects['N0'].name = 'n_' + bpy.data.objects['Armature'].pose.bones[0].name
bpy.data.objects.new('N1', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N1'])
bpy.data.objects['N1'].name = 'n_' + bpy.data.objects['Armature'].pose.bones[1].name
bpy.data.objects.new('N2', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N2'])
bpy.data.objects['N2'].name = 'n_' + bpy.data.objects['Armature'].pose.bones[2].name
bpy.data.objects.new('N3', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N3'])
bpy.data.objects['N3'].name = 'n_' + bpy.data.objects['Armature'].pose.bones[3].name
bpy.data.objects.new('N4', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N4'])
bpy.data.objects['N4'].name = 'n_' + bpy.data.objects['Armature'].pose.bones[4].name
bpy.data.objects.new('N5', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N5'])
bpy.data.objects['N5'].name = 'n_' + bpy.data.objects['Armature'].pose.bones[5].name
bpy.data.objects.new('N6', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N6'])
bpy.data.objects['N6'].name = 'n_' + bpy.data.objects['Armature'].pose.bones[6].name
bpy.data.objects.new('N7', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N7'])
bpy.data.objects['N7'].name = 'n_' + bpy.data.objects['Armature'].pose.bones[7].name
bpy.data.objects.new('N8', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N8'])
bpy.data.objects['N8'].name = 'n_' + bpy.data.objects['Armature'].pose.bones[8].name
bpy.data.objects.new('N9', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N9'])
bpy.data.objects['N9'].name = 'n_' + bpy.data.objects['Armature'].pose.bones[9].name
bpy.data.objects.new('N10', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N10'])
bpy.data.objects['N10'].name = 'n_' + bpy.data.objects['Armature'].pose.bones[10].name
bpy.data.objects.new('N11', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N11'])
bpy.data.objects['N11'].name = 'n_' + bpy.data.actions['ArmatureAction'].layers[0].strips[0].channelbags[0].fcurves[0].data_path
bpy.ops.pose.select_all(action='DESELECT')
bpy.data.objects['Armature'].pose.bones['Bone.L.001'].select = True
bpy.ops.pose.flip_names(do_strip_numbers=True)
bpy.data.objects.new('N12', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N12'])
bpy.data.objects['N12'].name = 'n_' + bpy.data.objects['Armature'].pose.bones[8].name
bpy.ops.pose.select_all(action='DESELECT')
bpy.data.objects['Armature'].pose.bones['tip'].select = True
bpy.data.objects['Armature'].pose.bones['mid'].select = True
bpy.data.objects['Armature'].pose.bones['B1'].select = True
bpy.data.objects['Armature'].pose.bones['Bone'].select = True
bpy.data.objects['Armature'].pose.bones['mid'].hide = True
bpy.ops.pose.autoside_names()
bpy.data.objects.new('N13', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N13'])
bpy.data.objects['N13'].name = 'n_' + bpy.data.objects['Armature'].pose.bones[0].name
bpy.data.objects.new('N14', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N14'])
bpy.data.objects['N14'].name = 'n_' + bpy.data.objects['Armature'].pose.bones[1].name
bpy.data.objects.new('N15', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N15'])
bpy.data.objects['N15'].name = 'n_' + bpy.data.objects['Armature'].pose.bones[2].name
bpy.data.objects.new('N16', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N16'])
bpy.data.objects['N16'].name = 'n_' + bpy.data.objects['Armature'].pose.bones[3].name
bpy.data.objects.new('N17', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N17'])
bpy.data.objects['N17'].name = 'n_' + bpy.data.objects['Armature'].pose.bones[4].name
bpy.data.objects.new('N18', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N18'])
bpy.data.objects['N18'].name = 'n_' + bpy.data.objects['Armature'].pose.bones[5].name
bpy.data.objects.new('N19', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N19'])
bpy.data.objects['N19'].name = 'n_' + bpy.data.objects['Armature'].pose.bones[6].name
bpy.data.objects.new('N20', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N20'])
bpy.data.objects['N20'].name = 'n_' + bpy.data.objects['Armature'].pose.bones[7].name
bpy.data.objects.new('N21', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N21'])
bpy.data.objects['N21'].name = 'n_' + bpy.data.objects['Armature'].pose.bones[8].name
bpy.data.objects.new('N22', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N22'])
bpy.data.objects['N22'].name = 'n_' + bpy.data.objects['Armature'].pose.bones[9].name
bpy.data.objects.new('N23', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N23'])
bpy.data.objects['N23'].name = 'n_' + bpy.data.objects['Armature'].pose.bones[10].name
bpy.data.objects['Armature'].pose.bones[10].hide = False
bpy.ops.pose.select_all(action='DESELECT')
bpy.data.objects['Armature'].pose.bones[9].select = True
bpy.data.objects['Armature'].pose.bones[10].select = True
bpy.ops.pose.autoside_names(axis='YAXIS')
bpy.data.objects.new('N24', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N24'])
bpy.data.objects['N24'].name = 'n_' + bpy.data.objects['Armature'].pose.bones[9].name
bpy.data.objects.new('N25', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N25'])
bpy.data.objects['N25'].name = 'n_' + bpy.data.objects['Armature'].pose.bones[10].name
bpy.ops.pose.autoside_names(axis='ZAXIS')
bpy.data.objects.new('N26', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N26'])
bpy.data.objects['N26'].name = 'n_' + bpy.data.objects['Armature'].pose.bones[9].name
bpy.data.objects.new('N27', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['N27'])
bpy.data.objects['N27'].name = 'n_' + bpy.data.objects['Armature'].pose.bones[10].name
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.pose.flip_names()
bpy.ops.pose.autoside_names()
