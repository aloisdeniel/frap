# WP 8.5: bone heat that finds no solution. A box with one NaN vertex makes the Laplacian
# singular: Blender reports the WARNING "Bone Heat Weighting: failed to find solution for one
# or more bones" (reports are not compared; frap's unit test pins it), the operator still
# finishes, the groups exist and stay empty, and the Armature modifier and parent are set.
# A healthy mesh parented in the same call gets its weights. The NaN is reset before the dump
# (GoldenState positions are finite). Weights are copied into X offsets (DISPLACE along X,
# mid_level 0: offset = weight).
# golden: startup=empty tier=T1 tol=1e-4 compare=objects,evaluated:W.Bad,evaluated:W.Good
import bpy
bpy.ops.object.armature_add(location=(0, 0, -1))
bpy.ops.object.mode_set(mode='EDIT')
bpy.data.armatures['Armature'].edit_bones['Bone'].tail = (0, 0, 0)
bpy.data.armatures['Armature'].edit_bones.new('Up')
bpy.data.armatures['Armature'].edit_bones['Up'].head = (0, 0, 0)
bpy.data.armatures['Armature'].edit_bones['Up'].tail = (0, 0, 1)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_cube_add(size=1.5)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.subdivide(number_cuts=2)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.object.name = 'Bad'
bpy.data.meshes['Cube'].vertices[5].co = (float('nan'), -0.75, 0.75)
bpy.ops.mesh.primitive_cube_add(size=1.5, location=(3, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.subdivide(number_cuts=2)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.object.name = 'Good'
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Bad'].select_set(True)
bpy.data.objects['Good'].select_set(True)
bpy.data.objects['Armature'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Armature']
bpy.ops.object.parent_set(type='ARMATURE_AUTO')
bpy.data.objects['Bad'].vertex_groups.keys()
#@blender assert bpy.data.objects['Bad'].vertex_groups.keys() == ['Bone', 'Up']
bpy.data.meshes['Cube'].vertices[5].co = (0.75, -0.75, 0.75)
bpy.data.objects.new('W.Bad', bpy.data.meshes['Cube'])
bpy.context.scene.collection.objects.link(bpy.data.objects['W.Bad'])
bpy.data.objects['W.Bad'].modifiers.new(name='D', type='DISPLACE')
bpy.data.objects['W.Bad'].modifiers['D'].vertex_group = 'Bone'
bpy.data.objects['W.Bad'].modifiers['D'].mid_level = 0.0
bpy.data.objects['W.Bad'].modifiers['D'].direction = 'X'
bpy.data.objects['W.Bad'].modifiers.new(name='U', type='DISPLACE')
bpy.data.objects['W.Bad'].modifiers['U'].vertex_group = 'Up'
bpy.data.objects['W.Bad'].modifiers['U'].mid_level = 0.0
bpy.data.objects['W.Bad'].modifiers['U'].direction = 'Y'
bpy.data.objects.new('W.Good', bpy.data.meshes['Cube.001'])
bpy.context.scene.collection.objects.link(bpy.data.objects['W.Good'])
bpy.data.objects['W.Good'].modifiers.new(name='D', type='DISPLACE')
bpy.data.objects['W.Good'].modifiers['D'].vertex_group = 'Bone'
bpy.data.objects['W.Good'].modifiers['D'].mid_level = 0.0
bpy.data.objects['W.Good'].modifiers['D'].direction = 'X'
bpy.data.objects['W.Good'].modifiers.new(name='U', type='DISPLACE')
bpy.data.objects['W.Good'].modifiers['U'].vertex_group = 'Up'
bpy.data.objects['W.Good'].modifiers['U'].mid_level = 0.0
bpy.data.objects['W.Good'].modifiers['U'].direction = 'Y'
