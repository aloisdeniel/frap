# WP 6.5: ARMATURE modifier linear blend skinning with vertex groups. A three-bone rig (a
# connected chain and a branch with a non-deforming bone) on a moved, rotated and scaled
# armature object; a UV sphere parented with parent_set(type='ARMATURE') (an Armature modifier,
# parent_type stays OBJECT) and weighted per vertex range: weights that do not sum to 1, a
# vertex in two groups, a group named after the non-deforming bone, a group without a bone,
# unweighted vertices. A second sphere is deformed by a modifier added with modifiers.new and
# no parent. Posed with rotations, scale and location in several rotation modes.
# golden: startup=factory tier=T2 tol=1e-5 compare=objects,evaluated:Sphere,evaluated:Sphere.001
import bpy
bpy.ops.object.armature_add(location=(0.3, -0.2, 0.1), rotation=(0.1, 0.2, -0.3))
bpy.ops.object.mode_set(mode='EDIT')
bpy.data.armatures['Armature'].edit_bones['Bone'].tail = (0, 0.1, 1.0)
bpy.data.armatures['Armature'].edit_bones['Bone'].roll = 0.2
bpy.data.armatures['Armature'].edit_bones.new('Upper')
bpy.data.armatures['Armature'].edit_bones['Upper'].head = (0, 0.1, 1.0)
bpy.data.armatures['Armature'].edit_bones['Upper'].tail = (0.1, 0.0, 2.0)
bpy.data.armatures['Armature'].edit_bones['Upper'].parent = bpy.data.armatures['Armature'].edit_bones['Bone']
bpy.data.armatures['Armature'].edit_bones['Upper'].use_connect = True
bpy.data.armatures['Armature'].edit_bones.new('Side')
bpy.data.armatures['Armature'].edit_bones['Side'].head = (0, 0.1, 1.0)
bpy.data.armatures['Armature'].edit_bones['Side'].tail = (1.0, 0.1, 1.2)
bpy.data.armatures['Armature'].edit_bones['Side'].parent = bpy.data.armatures['Armature'].edit_bones['Bone']
bpy.data.armatures['Armature'].edit_bones.new('Helper')
bpy.data.armatures['Armature'].edit_bones['Helper'].head = (0, -1, 0)
bpy.data.armatures['Armature'].edit_bones['Helper'].tail = (0, -1, 1)
bpy.data.armatures['Armature'].edit_bones['Helper'].use_deform = False
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_uv_sphere_add(segments=8, ring_count=6, radius=1.0, location=(0.2, 0, 1.2), scale=(0.6, 0.6, 1.2))
bpy.data.objects['Sphere'].rotation_euler = (0, 0, 0.3)
bpy.data.objects['Sphere'].vertex_groups.new(name='Bone')
bpy.data.objects['Sphere'].vertex_groups.new(name='Upper')
bpy.data.objects['Sphere'].vertex_groups.new(name='Side')
bpy.data.objects['Sphere'].vertex_groups.new(name='Helper')
bpy.data.objects['Sphere'].vertex_groups.new(name='Nobody')
bpy.data.objects['Sphere'].vertex_groups['Bone'].add([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19], 1.0, 'REPLACE')
bpy.data.objects['Sphere'].vertex_groups['Upper'].add([12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41], 0.6, 'REPLACE')
bpy.data.objects['Sphere'].vertex_groups['Side'].add([3, 4, 5, 20, 21, 22, 23], 0.3, 'REPLACE')
bpy.data.objects['Sphere'].vertex_groups['Helper'].add([6, 7, 24], 0.8, 'REPLACE')
bpy.data.objects['Sphere'].vertex_groups['Nobody'].add([8, 25, 40], 0.5, 'REPLACE')
bpy.data.objects['Sphere'].vertex_groups['Bone'].remove([9])
bpy.ops.object.duplicate()
bpy.data.objects['Sphere.001'].location = (-1.5, 0.5, 0.5)
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Sphere'].select_set(True)
bpy.data.objects['Armature'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Armature']
bpy.ops.object.parent_set(type='ARMATURE')
bpy.data.objects['Sphere'].parent_type
#@blender assert bpy.data.objects['Sphere'].parent_type == 'OBJECT'
bpy.data.objects['Sphere'].modifiers['Armature'].object.name
bpy.data.objects['Sphere.001'].modifiers.new('Rig', 'ARMATURE')
bpy.data.objects['Sphere.001'].modifiers['Rig'].object = bpy.data.objects['Armature']
bpy.data.objects['Armature'].pose.bones['Bone'].rotation_quaternion = (0.9, 0.2, -0.1, 0.3)
bpy.data.objects['Armature'].pose.bones['Bone'].location = (0.1, 0.0, -0.2)
bpy.data.objects['Armature'].pose.bones['Upper'].rotation_mode = 'XZY'
bpy.data.objects['Armature'].pose.bones['Upper'].rotation_euler = (0.6, -0.3, 0.4)
bpy.data.objects['Armature'].pose.bones['Upper'].scale = (1.2, 0.8, 1.1)
bpy.data.objects['Armature'].pose.bones['Side'].rotation_mode = 'AXIS_ANGLE'
bpy.data.objects['Armature'].pose.bones['Side'].rotation_axis_angle = (0.7, 0.2, 0.3, 1.0)
bpy.data.objects['Armature'].pose.bones['Side'].location = (0.0, 0.3, 0.1)
bpy.data.objects['Armature'].pose.bones['Helper'].location = (0.5, 0.5, 0.5)
bpy.data.objects['Armature'].location = (0.4, -0.1, 0.3)
bpy.data.objects['Armature'].scale = (1.1, 1.1, 0.9)
