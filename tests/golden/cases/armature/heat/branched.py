# WP 8.5: object.parent_set(type='ARMATURE_AUTO') on Suzanne (several disconnected parts:
# the head, the eyes; triangles and quads) with a branched rig: Spine, a Head child, two ear
# bones and a jaw bone branching off the head. Heat sources compete between the branches and
# the visibility rays decide which bones the eyes and ears see. Weights are copied into X
# offsets of objects sharing the mesh (DISPLACE along X, mid_level 0: offset = weight).
# golden: startup=empty tier=T1 tol=1e-4 compare=objects,evaluated:Suzanne,evaluated:W.Spine,evaluated:W.Head,evaluated:W.Ear.L,evaluated:W.Ear.R,evaluated:W.Jaw
import bpy
bpy.ops.object.armature_add(location=(0, 0, -1.2))
bpy.ops.object.mode_set(mode='EDIT')
bpy.data.armatures['Armature'].edit_bones['Bone'].name = 'Spine'
bpy.data.armatures['Armature'].edit_bones['Spine'].tail = (0, 0, 0.8)
bpy.data.armatures['Armature'].edit_bones.new('Head')
bpy.data.armatures['Armature'].edit_bones['Head'].head = (0, 0, 0.8)
bpy.data.armatures['Armature'].edit_bones['Head'].tail = (0, 0, 1.9)
bpy.data.armatures['Armature'].edit_bones['Head'].parent = bpy.data.armatures['Armature'].edit_bones['Spine']
bpy.data.armatures['Armature'].edit_bones.new('Ear.L')
bpy.data.armatures['Armature'].edit_bones['Ear.L'].head = (0.6, 0.1, 1.3)
bpy.data.armatures['Armature'].edit_bones['Ear.L'].tail = (1.3, 0.2, 1.5)
bpy.data.armatures['Armature'].edit_bones['Ear.L'].parent = bpy.data.armatures['Armature'].edit_bones['Head']
bpy.data.armatures['Armature'].edit_bones.new('Ear.R')
bpy.data.armatures['Armature'].edit_bones['Ear.R'].head = (-0.6, 0.1, 1.3)
bpy.data.armatures['Armature'].edit_bones['Ear.R'].tail = (-1.3, 0.2, 1.5)
bpy.data.armatures['Armature'].edit_bones['Ear.R'].parent = bpy.data.armatures['Armature'].edit_bones['Head']
bpy.data.armatures['Armature'].edit_bones.new('Jaw')
bpy.data.armatures['Armature'].edit_bones['Jaw'].head = (0, 0.1, 0.9)
bpy.data.armatures['Armature'].edit_bones['Jaw'].tail = (0, -0.7, 0.6)
bpy.data.armatures['Armature'].edit_bones['Jaw'].parent = bpy.data.armatures['Armature'].edit_bones['Head']
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_monkey_add(size=2.0, location=(0, 0, 0.1), rotation=(0.05, 0, 0))
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Suzanne'].select_set(True)
bpy.data.objects['Armature'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Armature']
bpy.ops.object.parent_set(type='ARMATURE_AUTO')
bpy.data.objects['Suzanne'].vertex_groups.keys()
#@blender assert bpy.data.objects['Suzanne'].vertex_groups.keys() == ['Spine', 'Head', 'Ear.L', 'Ear.R', 'Jaw']
bpy.data.objects.new('W.Spine', bpy.data.meshes['Suzanne'])
bpy.context.scene.collection.objects.link(bpy.data.objects['W.Spine'])
bpy.data.objects['W.Spine'].modifiers.new(name='D', type='DISPLACE')
bpy.data.objects['W.Spine'].modifiers['D'].vertex_group = 'Spine'
bpy.data.objects['W.Spine'].modifiers['D'].mid_level = 0.0
bpy.data.objects['W.Spine'].modifiers['D'].direction = 'X'
bpy.data.objects.new('W.Head', bpy.data.meshes['Suzanne'])
bpy.context.scene.collection.objects.link(bpy.data.objects['W.Head'])
bpy.data.objects['W.Head'].modifiers.new(name='D', type='DISPLACE')
bpy.data.objects['W.Head'].modifiers['D'].vertex_group = 'Head'
bpy.data.objects['W.Head'].modifiers['D'].mid_level = 0.0
bpy.data.objects['W.Head'].modifiers['D'].direction = 'X'
bpy.data.objects.new('W.Ear.L', bpy.data.meshes['Suzanne'])
bpy.context.scene.collection.objects.link(bpy.data.objects['W.Ear.L'])
bpy.data.objects['W.Ear.L'].modifiers.new(name='D', type='DISPLACE')
bpy.data.objects['W.Ear.L'].modifiers['D'].vertex_group = 'Ear.L'
bpy.data.objects['W.Ear.L'].modifiers['D'].mid_level = 0.0
bpy.data.objects['W.Ear.L'].modifiers['D'].direction = 'X'
bpy.data.objects.new('W.Ear.R', bpy.data.meshes['Suzanne'])
bpy.context.scene.collection.objects.link(bpy.data.objects['W.Ear.R'])
bpy.data.objects['W.Ear.R'].modifiers.new(name='D', type='DISPLACE')
bpy.data.objects['W.Ear.R'].modifiers['D'].vertex_group = 'Ear.R'
bpy.data.objects['W.Ear.R'].modifiers['D'].mid_level = 0.0
bpy.data.objects['W.Ear.R'].modifiers['D'].direction = 'X'
bpy.data.objects.new('W.Jaw', bpy.data.meshes['Suzanne'])
bpy.context.scene.collection.objects.link(bpy.data.objects['W.Jaw'])
bpy.data.objects['W.Jaw'].modifiers.new(name='D', type='DISPLACE')
bpy.data.objects['W.Jaw'].modifiers['D'].vertex_group = 'Jaw'
bpy.data.objects['W.Jaw'].modifiers['D'].mid_level = 0.0
bpy.data.objects['W.Jaw'].modifiers['D'].direction = 'X'
bpy.data.objects['Armature'].pose.bones['Head'].rotation_quaternion = (0.97, 0.2, 0.0, 0.1)
bpy.data.objects['Armature'].pose.bones['Ear.L'].rotation_quaternion = (0.9, 0.0, 0.43, 0.0)
bpy.data.objects['Armature'].pose.bones['Jaw'].rotation_quaternion = (0.95, -0.3, 0.0, 0.0)
