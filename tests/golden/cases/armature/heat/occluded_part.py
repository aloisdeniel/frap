# WP 8.5: bone heat with a fully occluded mesh part. One mesh: a subdivided box with a
# two-bone chain inside, plus a separate rotated icosphere "hat" above it, added in Edit Mode.
# Every visibility ray from a hat vertex to a bone hits the box's top face, so the hat has no
# heat source (H = 0) and its Laplacian block is singular. Blender's Eigen SparseLU meets no
# exactly zero pivot here: the solve succeeds, the box gets its weights and the hat none. (A
# plain axis-aligned cube or plane hat makes Eigen meet an exactly zero pivot instead and the
# whole mesh stays unweighted, with the failure WARNING; frap does not reproduce that pivot
# outcome: deviations §2.) Weights are copied into X offsets (DISPLACE along X, mid_level 0:
# offset = weight).
# golden: startup=empty tier=T1 tol=1e-4 compare=objects,evaluated:W.Bone,evaluated:W.Top
import bpy
bpy.ops.object.armature_add(location=(0, 0, -0.3))
bpy.ops.object.mode_set(mode='EDIT')
bpy.data.armatures['Armature'].edit_bones['Bone'].tail = (0, 0, 0.0)
bpy.data.armatures['Armature'].edit_bones.new('Top')
bpy.data.armatures['Armature'].edit_bones['Top'].head = (0, 0, 0.0)
bpy.data.armatures['Armature'].edit_bones['Top'].tail = (0, 0, 0.3)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_cube_add(size=2.0)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.subdivide(number_cuts=1)
bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=2, radius=0.3, location=(0.05, 0.1, 2.0), rotation=(0.3, 0.7, 0.1))
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Cube'].select_set(True)
bpy.data.objects['Armature'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Armature']
bpy.ops.object.parent_set(type='ARMATURE_AUTO')
bpy.data.objects.new('W.Bone', bpy.data.meshes['Cube'])
bpy.context.scene.collection.objects.link(bpy.data.objects['W.Bone'])
bpy.data.objects['W.Bone'].modifiers.new(name='D', type='DISPLACE')
bpy.data.objects['W.Bone'].modifiers['D'].vertex_group = 'Bone'
bpy.data.objects['W.Bone'].modifiers['D'].mid_level = 0.0
bpy.data.objects['W.Bone'].modifiers['D'].direction = 'X'
bpy.data.objects.new('W.Top', bpy.data.meshes['Cube'])
bpy.context.scene.collection.objects.link(bpy.data.objects['W.Top'])
bpy.data.objects['W.Top'].modifiers.new(name='D', type='DISPLACE')
bpy.data.objects['W.Top'].modifiers['D'].vertex_group = 'Top'
bpy.data.objects['W.Top'].modifiers['D'].mid_level = 0.0
bpy.data.objects['W.Top'].modifiers['D'].direction = 'X'
