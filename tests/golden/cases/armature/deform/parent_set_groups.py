# WP 6.5: object.parent_set ARMATURE_NAME and ARMATURE_ENVELOPE. A symmetric rig (Spine, Arm.L,
# Arm.R, a non-deforming Root) on a moved, scaled armature. ARMATURE_NAME adds an empty group
# per deforming bone in bone order after the existing ones, skips existing names, drops weights
# of out-of-range groups, and makes the last new group active. ARMATURE_ENVELOPE fills the
# groups with envelope weights in world space (radii scaled by the armature's scale), keeps a
# locked group, and xmirror mirrors the weights of each side onto the other. Weights are copied
# into empties' locations (objects); the posed meshes are compared (evaluated:).
# golden: startup=factory tier=T1 tol=1e-5 compare=objects,evaluated:Plane,evaluated:Plane.001,evaluated:Plane.002
import bpy
bpy.ops.object.armature_add(location=(0, 0.1, 0), scale=(1.2, 1.2, 1.2))
bpy.ops.object.mode_set(mode='EDIT')
bpy.data.armatures['Armature'].edit_bones['Bone'].name = 'Root'
bpy.data.armatures['Armature'].edit_bones['Root'].use_deform = False
bpy.data.armatures['Armature'].edit_bones['Root'].tail = (0, 0, 0.5)
bpy.data.armatures['Armature'].edit_bones.new('Spine')
bpy.data.armatures['Armature'].edit_bones['Spine'].head = (0, 0, 0.5)
bpy.data.armatures['Armature'].edit_bones['Spine'].tail = (0, 0, 1.5)
bpy.data.armatures['Armature'].edit_bones['Spine'].parent = bpy.data.armatures['Armature'].edit_bones['Root']
bpy.data.armatures['Armature'].edit_bones['Spine'].envelope_distance = 0.4
bpy.data.armatures['Armature'].edit_bones.new('Arm.L')
bpy.data.armatures['Armature'].edit_bones['Arm.L'].head = (0.2, 0, 1.3)
bpy.data.armatures['Armature'].edit_bones['Arm.L'].tail = (1.2, 0, 1.3)
bpy.data.armatures['Armature'].edit_bones['Arm.L'].parent = bpy.data.armatures['Armature'].edit_bones['Spine']
bpy.data.armatures['Armature'].edit_bones['Arm.L'].head_radius = 0.2
bpy.data.armatures['Armature'].edit_bones['Arm.L'].tail_radius = 0.15
bpy.data.armatures['Armature'].edit_bones.new('Arm.R')
bpy.data.armatures['Armature'].edit_bones['Arm.R'].head = (-0.2, 0, 1.3)
bpy.data.armatures['Armature'].edit_bones['Arm.R'].tail = (-1.2, 0, 1.3)
bpy.data.armatures['Armature'].edit_bones['Arm.R'].parent = bpy.data.armatures['Armature'].edit_bones['Spine']
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_plane_add(size=1.0, location=(0, 0, 1.0), rotation=(1.5707964, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.subdivide(number_cuts=6)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Plane'].scale = (2.6, 1.0, 1.4)
bpy.ops.object.duplicate()
bpy.ops.object.duplicate()
bpy.data.objects['Plane'].vertex_groups.new(name='Spine')
bpy.data.objects['Plane'].vertex_groups.new(name='Extra')
bpy.data.objects['Plane'].vertex_groups['Extra'].add([0, 1, 2], 0.5, 'REPLACE')
bpy.data.objects['Plane'].vertex_groups['Spine'].add([3, 4], 1.0, 'REPLACE')
bpy.data.objects['Plane'].vertex_groups.remove(bpy.data.objects['Plane'].vertex_groups['Extra'])
bpy.data.objects['Plane'].vertex_groups.new(name='Extra')
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Plane'].select_set(True)
bpy.data.objects['Armature'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Armature']
bpy.ops.object.parent_set(type='ARMATURE_NAME')
bpy.data.objects['Plane'].vertex_groups.keys()
#@blender assert bpy.data.objects['Plane'].vertex_groups.keys() == ['Spine', 'Extra', 'Arm.L', 'Arm.R']
bpy.data.objects['Plane'].vertex_groups.active_index
#@blender assert bpy.data.objects['Plane'].vertex_groups.active_index == 3
bpy.data.objects['Plane.001'].vertex_groups.new(name='Arm.R')
bpy.data.objects['Plane.001'].vertex_groups['Arm.R'].add([0, 1, 2, 3], 0.25, 'REPLACE')
bpy.data.objects['Plane.001'].vertex_groups['Arm.R'].lock_weight = True
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Plane.001'].select_set(True)
bpy.data.objects['Armature'].select_set(True)
bpy.ops.object.parent_set(type='ARMATURE_ENVELOPE')
bpy.data.objects['Plane.001'].vertex_groups.keys()
#@blender assert bpy.data.objects['Plane.001'].vertex_groups.keys() == ['Arm.R', 'Spine', 'Arm.L']
bpy.data.objects['Plane.001'].vertex_groups.active_index
#@blender assert bpy.data.objects['Plane.001'].vertex_groups.active_index == 2
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Plane.002'].select_set(True)
bpy.data.objects['Armature'].select_set(True)
bpy.data.objects['Plane.002'].location.x = 0.15
bpy.data.objects['Plane.002'].vertex_groups.new(name='Arm.R')
bpy.ops.object.parent_set(type='ARMATURE_ENVELOPE', xmirror=True)
bpy.data.objects['Plane.002'].vertex_groups.keys()
bpy.data.objects.new('W0', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['W0'])
bpy.data.objects['W0'].location = (bpy.data.objects['Plane.001'].vertex_groups['Spine'].weight(12), bpy.data.objects['Plane.001'].vertex_groups['Spine'].weight(24), bpy.data.objects['Plane.001'].vertex_groups['Spine'].weight(44))
bpy.data.objects.new('W1', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['W1'])
bpy.data.objects['W1'].location = (bpy.data.objects['Plane.001'].vertex_groups['Arm.L'].weight(18), bpy.data.objects['Plane.001'].vertex_groups['Arm.L'].weight(45), bpy.data.objects['Plane.001'].vertex_groups['Arm.R'].weight(0))
bpy.data.objects.new('W2', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['W2'])
bpy.data.objects['W2'].location = (bpy.data.objects['Plane.002'].vertex_groups['Arm.R'].weight(4), bpy.data.objects['Plane.002'].vertex_groups['Arm.L'].weight(19), bpy.data.objects['Plane.002'].vertex_groups['Spine'].weight(23))
bpy.data.objects.new('W3', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['W3'])
bpy.data.objects['W3'].location = (bpy.data.objects['Plane.002'].vertex_groups['Arm.R'].weight(30), bpy.data.objects['Plane.002'].vertex_groups['Arm.L'].weight(60), bpy.data.objects['Plane.002'].vertex_groups['Spine'].weight(45))
bpy.data.objects['Plane.001'].vertex_groups['Arm.L'].weight(0)
bpy.data.objects['Plane.002'].vertex_groups['Arm.L'].weight(0)
bpy.data.objects['Armature'].pose.bones['Arm.L'].rotation_quaternion = (0.9, 0.0, 0.0, 0.4)
bpy.data.objects['Armature'].pose.bones['Arm.R'].rotation_quaternion = (0.9, 0.3, 0.0, -0.4)
bpy.data.objects['Armature'].pose.bones['Spine'].rotation_quaternion = (0.98, 0.15, 0.0, 0.0)
bpy.data.objects['Armature'].pose.bones['Root'].location = (0.0, 0.3, 0.0)
