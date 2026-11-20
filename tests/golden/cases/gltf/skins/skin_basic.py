# WP 6.8: a skinned mesh. A four-bone rig (a connected chain, a branch, a bone at the root) on
# a moved and rotated armature object; a UV sphere parented to it with an ARMATURE modifier,
# weighted per vertex range (weights that do not sum to 1, a vertex in five groups, a group
# without a bone, a weight below 0.0001); the rig posed. Default export: joints in the rest
# pose, inverse bind matrices, JOINTS_0/WEIGHTS_0 normalised to the 4 heaviest influences,
# the mesh in world space, the skinned node without TRS.
# golden: startup=factory tier=T2 tol=1e-5 compare=glb:out/skin_basic.glb
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
bpy.data.armatures['Armature'].edit_bones['Side'].roll = -0.5
bpy.data.armatures['Armature'].edit_bones['Side'].parent = bpy.data.armatures['Armature'].edit_bones['Bone']
bpy.data.armatures['Armature'].edit_bones.new('Helper')
bpy.data.armatures['Armature'].edit_bones['Helper'].head = (0, -1, 0)
bpy.data.armatures['Armature'].edit_bones['Helper'].tail = (0, -1, 1)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_uv_sphere_add(segments=8, ring_count=6, radius=1.0, location=(0.2, 0, 1.2), scale=(0.6, 0.6, 1.2))
bpy.data.objects['Sphere'].rotation_euler = (0, 0, 0.3)
bpy.data.objects['Sphere'].vertex_groups.new(name='Bone')
bpy.data.objects['Sphere'].vertex_groups.new(name='Upper')
bpy.data.objects['Sphere'].vertex_groups.new(name='Side')
bpy.data.objects['Sphere'].vertex_groups.new(name='Helper')
bpy.data.objects['Sphere'].vertex_groups.new(name='Nobody')
bpy.data.objects['Sphere'].vertex_groups['Bone'].add([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21], 1.0, 'REPLACE')
bpy.data.objects['Sphere'].vertex_groups['Upper'].add([12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41], 0.6, 'REPLACE')
bpy.data.objects['Sphere'].vertex_groups['Side'].add([3, 4, 5, 20, 21, 22, 23], 0.3, 'REPLACE')
bpy.data.objects['Sphere'].vertex_groups['Helper'].add([6, 7, 20, 21, 24], 0.8, 'REPLACE')
bpy.data.objects['Sphere'].vertex_groups['Nobody'].add([8, 21, 25, 40], 0.5, 'REPLACE')
bpy.data.objects['Sphere'].vertex_groups['Side'].add([30], 0.00005, 'REPLACE')
bpy.data.objects['Sphere'].modifiers.new(name='Armature', type='ARMATURE')
bpy.data.objects['Sphere'].modifiers['Armature'].object = bpy.data.objects['Armature']
bpy.data.objects['Sphere'].parent = bpy.data.objects['Armature']
bpy.data.objects['Armature'].pose.bones['Bone'].rotation_quaternion = (0.95, 0.1, 0.2, 0.0)
bpy.data.objects['Armature'].pose.bones['Upper'].rotation_mode = 'XYZ'
bpy.data.objects['Armature'].pose.bones['Upper'].rotation_euler = (0.4, 0.0, -0.3)
bpy.data.objects['Armature'].pose.bones['Side'].location = (0.1, 0.2, 0.0)
bpy.data.objects['Armature'].pose.bones['Side'].scale = (1.0, 1.5, 0.8)
bpy.ops.export_scene.gltf(filepath='out/skin_basic.glb')
