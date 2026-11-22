# WP 8.5: object.parent_set(type='ARMATURE_AUTO') with every bone outside the mesh (a
# subdivided box, rotated): one beside +X, one beside -X tilted, one high above, plus a bone
# whose group exists and is locked (left untouched). Vertices on the far side of the box do not
# see a bone through the near faces (the visibility rays hit their fronts), so the heat sources
# follow the visible bones. Weights are copied into X offsets (DISPLACE along X, mid_level 0:
# offset = weight).
# golden: startup=empty tier=T1 tol=1e-4 compare=objects,evaluated:Cube,evaluated:W.Side.L,evaluated:W.Side.R,evaluated:W.Top,evaluated:W.Locked
import bpy
bpy.ops.object.armature_add(location=(0, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.data.armatures['Armature'].edit_bones['Bone'].name = 'Side.L'
bpy.data.armatures['Armature'].edit_bones['Side.L'].head = (1.5, 0, -0.8)
bpy.data.armatures['Armature'].edit_bones['Side.L'].tail = (1.6, 0.1, 0.8)
bpy.data.armatures['Armature'].edit_bones.new('Side.R')
bpy.data.armatures['Armature'].edit_bones['Side.R'].head = (-1.4, 0.3, -0.6)
bpy.data.armatures['Armature'].edit_bones['Side.R'].tail = (-2.0, -0.2, 0.9)
bpy.data.armatures['Armature'].edit_bones.new('Top')
bpy.data.armatures['Armature'].edit_bones['Top'].head = (0.2, 0, 2.5)
bpy.data.armatures['Armature'].edit_bones['Top'].tail = (0.2, 0, 3.5)
bpy.data.armatures['Armature'].edit_bones.new('Locked')
bpy.data.armatures['Armature'].edit_bones['Locked'].head = (0, -1.5, 0)
bpy.data.armatures['Armature'].edit_bones['Locked'].tail = (0, -2.5, 0)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_cube_add(size=1.6, location=(0.05, 0, 0.1), rotation=(0.1, 0.2, 0.3))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.subdivide(number_cuts=3)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Cube'].vertex_groups.new(name='Locked')
bpy.data.objects['Cube'].vertex_groups['Locked'].add([0, 1, 2, 3], 0.4, 'REPLACE')
bpy.data.objects['Cube'].vertex_groups['Locked'].lock_weight = True
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Cube'].select_set(True)
bpy.data.objects['Armature'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Armature']
bpy.ops.object.parent_set(type='ARMATURE_AUTO')
bpy.data.objects['Cube'].vertex_groups.keys()
#@blender assert bpy.data.objects['Cube'].vertex_groups.keys() == ['Locked', 'Side.L', 'Side.R', 'Top']
bpy.data.objects['Cube'].vertex_groups.active_index
#@blender assert bpy.data.objects['Cube'].vertex_groups.active_index == 3
bpy.data.objects.new('W.Side.L', bpy.data.meshes['Cube'])
bpy.context.scene.collection.objects.link(bpy.data.objects['W.Side.L'])
bpy.data.objects['W.Side.L'].modifiers.new(name='D', type='DISPLACE')
bpy.data.objects['W.Side.L'].modifiers['D'].vertex_group = 'Side.L'
bpy.data.objects['W.Side.L'].modifiers['D'].mid_level = 0.0
bpy.data.objects['W.Side.L'].modifiers['D'].direction = 'X'
bpy.data.objects.new('W.Side.R', bpy.data.meshes['Cube'])
bpy.context.scene.collection.objects.link(bpy.data.objects['W.Side.R'])
bpy.data.objects['W.Side.R'].modifiers.new(name='D', type='DISPLACE')
bpy.data.objects['W.Side.R'].modifiers['D'].vertex_group = 'Side.R'
bpy.data.objects['W.Side.R'].modifiers['D'].mid_level = 0.0
bpy.data.objects['W.Side.R'].modifiers['D'].direction = 'X'
bpy.data.objects.new('W.Top', bpy.data.meshes['Cube'])
bpy.context.scene.collection.objects.link(bpy.data.objects['W.Top'])
bpy.data.objects['W.Top'].modifiers.new(name='D', type='DISPLACE')
bpy.data.objects['W.Top'].modifiers['D'].vertex_group = 'Top'
bpy.data.objects['W.Top'].modifiers['D'].mid_level = 0.0
bpy.data.objects['W.Top'].modifiers['D'].direction = 'X'
bpy.data.objects.new('W.Locked', bpy.data.meshes['Cube'])
bpy.context.scene.collection.objects.link(bpy.data.objects['W.Locked'])
bpy.data.objects['W.Locked'].modifiers.new(name='D', type='DISPLACE')
bpy.data.objects['W.Locked'].modifiers['D'].vertex_group = 'Locked'
bpy.data.objects['W.Locked'].modifiers['D'].mid_level = 0.0
bpy.data.objects['W.Locked'].modifiers['D'].direction = 'X'
bpy.data.objects['Armature'].pose.bones['Top'].location = (0.0, 0.3, 0.0)
bpy.data.objects['Armature'].pose.bones['Side.L'].rotation_quaternion = (0.95, 0.0, 0.3, 0.0)
