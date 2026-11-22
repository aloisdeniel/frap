# WP 8.5: object.parent_set(type='ARMATURE_AUTO', xmirror=True) with a one-sided rig on a mesh
# symmetric in its local X (a subdivided bar, the object moved along X): Spine (its name flips
# to itself) and Arm.L, with an existing Arm.R group. Each weight is written to the vertex and
# again to its spatial X mirror in the flipped group, in vertex order, so Arm.R receives
# Arm.L's weights mirrored and Spine's become symmetric as the later vertices overwrite.
# Weights are copied into X offsets (DISPLACE along X, mid_level 0: offset = weight).
# golden: startup=empty tier=T1 tol=1e-4 compare=objects,evaluated:Cube,evaluated:W.Spine,evaluated:W.Arm.L,evaluated:W.Arm.R
import bpy
bpy.ops.object.armature_add(location=(0, 0, -0.6))
bpy.ops.object.mode_set(mode='EDIT')
bpy.data.armatures['Armature'].edit_bones['Bone'].name = 'Spine'
bpy.data.armatures['Armature'].edit_bones['Spine'].tail = (0, 0, 1.0)
bpy.data.armatures['Armature'].edit_bones.new('Arm.L')
bpy.data.armatures['Armature'].edit_bones['Arm.L'].head = (0.3, 0, 0.6)
bpy.data.armatures['Armature'].edit_bones['Arm.L'].tail = (1.6, 0, 0.65)
bpy.data.armatures['Armature'].edit_bones['Arm.L'].parent = bpy.data.armatures['Armature'].edit_bones['Spine']
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_cube_add(size=1.0)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.subdivide(number_cuts=4)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Cube'].scale = (3.4, 0.5, 0.5)
bpy.data.objects['Cube'].location = (0.12, 0.02, 0.0)
bpy.data.objects['Cube'].vertex_groups.new(name='Arm.R')
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Cube'].select_set(True)
bpy.data.objects['Armature'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Armature']
bpy.ops.object.parent_set(type='ARMATURE_AUTO', xmirror=True)
bpy.data.objects['Cube'].vertex_groups.keys()
#@blender assert bpy.data.objects['Cube'].vertex_groups.keys() == ['Arm.R', 'Spine', 'Arm.L']
bpy.data.objects.new('W.Spine', bpy.data.meshes['Cube'])
bpy.context.scene.collection.objects.link(bpy.data.objects['W.Spine'])
bpy.data.objects['W.Spine'].modifiers.new(name='D', type='DISPLACE')
bpy.data.objects['W.Spine'].modifiers['D'].vertex_group = 'Spine'
bpy.data.objects['W.Spine'].modifiers['D'].mid_level = 0.0
bpy.data.objects['W.Spine'].modifiers['D'].direction = 'X'
bpy.data.objects.new('W.Arm.L', bpy.data.meshes['Cube'])
bpy.context.scene.collection.objects.link(bpy.data.objects['W.Arm.L'])
bpy.data.objects['W.Arm.L'].modifiers.new(name='D', type='DISPLACE')
bpy.data.objects['W.Arm.L'].modifiers['D'].vertex_group = 'Arm.L'
bpy.data.objects['W.Arm.L'].modifiers['D'].mid_level = 0.0
bpy.data.objects['W.Arm.L'].modifiers['D'].direction = 'X'
bpy.data.objects.new('W.Arm.R', bpy.data.meshes['Cube'])
bpy.context.scene.collection.objects.link(bpy.data.objects['W.Arm.R'])
bpy.data.objects['W.Arm.R'].modifiers.new(name='D', type='DISPLACE')
bpy.data.objects['W.Arm.R'].modifiers['D'].vertex_group = 'Arm.R'
bpy.data.objects['W.Arm.R'].modifiers['D'].mid_level = 0.0
bpy.data.objects['W.Arm.R'].modifiers['D'].direction = 'X'
bpy.data.objects['Armature'].pose.bones['Arm.L'].rotation_quaternion = (0.9, 0.0, 0.3, 0.3)
