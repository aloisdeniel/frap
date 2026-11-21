# WP 6.8: the skin options on one scene. A rig with a connected chain, a branch, a
# non-deforming root bone and a non-deforming bone with an object parented to it
# (parent_type BONE); a UV sphere skinned with up to six influences per vertex, parented to the
# armature, with an empty child; an object parented to the armature without a modifier.
# Exports: default; export_def_bones; export_hierarchy_flatten_bones; export_leaf_bone;
# export_rest_position_armature=False (joints in the current pose); export_influence_nb=2;
# export_all_influences; export_skins=False; export_yup=False; export_hierarchy_flatten_objs;
# export_armature_object_remove (refused: two root bones).
# golden: startup=factory tier=T2 tol=1e-5 compare=glb:out/default.glb,glb:out/def_bones.glb,glb:out/flatten_bones.glb,glb:out/leaf.glb,glb:out/pose.glb,glb:out/influence2.glb,glb:out/all_influences.glb,glb:out/no_skins.glb,glb:out/zup.glb,glb:out/flatten_objs.glb,glb:out/remove_refused.glb glb_structure
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
bpy.data.armatures['Armature'].edit_bones['Side'].use_deform = False
bpy.data.armatures['Armature'].edit_bones.new('Helper')
bpy.data.armatures['Armature'].edit_bones['Helper'].head = (0, -1, 0)
bpy.data.armatures['Armature'].edit_bones['Helper'].tail = (0, -1, 1)
bpy.data.armatures['Armature'].edit_bones['Helper'].use_deform = False
bpy.data.armatures['Armature'].edit_bones.new('Tip')
bpy.data.armatures['Armature'].edit_bones['Tip'].head = (0.1, 0.0, 2.0)
bpy.data.armatures['Armature'].edit_bones['Tip'].tail = (0.1, 0.3, 2.6)
bpy.data.armatures['Armature'].edit_bones['Tip'].parent = bpy.data.armatures['Armature'].edit_bones['Upper']
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_uv_sphere_add(segments=8, ring_count=6, radius=1.0, location=(0.2, 0, 1.2), scale=(0.6, 0.6, 1.2))
bpy.data.objects['Sphere'].vertex_groups.new(name='Bone')
bpy.data.objects['Sphere'].vertex_groups.new(name='Upper')
bpy.data.objects['Sphere'].vertex_groups.new(name='Side')
bpy.data.objects['Sphere'].vertex_groups.new(name='Helper')
bpy.data.objects['Sphere'].vertex_groups.new(name='Tip')
bpy.data.objects['Sphere'].vertex_groups['Bone'].add([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21], 0.9, 'REPLACE')
bpy.data.objects['Sphere'].vertex_groups['Upper'].add([12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41], 0.6, 'REPLACE')
bpy.data.objects['Sphere'].vertex_groups['Side'].add([3, 4, 5, 20, 21, 22, 23], 0.3, 'REPLACE')
bpy.data.objects['Sphere'].vertex_groups['Helper'].add([6, 7, 20, 21, 24], 0.8, 'REPLACE')
bpy.data.objects['Sphere'].vertex_groups['Tip'].add([20, 21, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41], 0.4, 'REPLACE')
bpy.data.objects['Sphere'].modifiers.new(name='Armature', type='ARMATURE')
bpy.data.objects['Sphere'].modifiers['Armature'].object = bpy.data.objects['Armature']
bpy.data.objects['Sphere'].parent = bpy.data.objects['Armature']
bpy.data.objects.new('Hat', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Hat'])
bpy.data.objects['Hat'].location = (0, 0.5, 2.5)
bpy.data.objects['Hat'].parent = bpy.data.objects['Sphere']
bpy.ops.mesh.primitive_cube_add(size=0.3, location=(1.0, 0.3, 1.5))
bpy.data.objects['Cube.001'].parent = bpy.data.objects['Armature']
bpy.data.objects['Cube.001'].parent_type = 'BONE'
bpy.data.objects['Cube.001'].parent_bone = 'Side'
bpy.data.objects['Cube.001'].location = (0.2, 0.1, -0.3)
bpy.ops.mesh.primitive_cone_add(vertices=5, radius1=0.2, depth=0.4, location=(-1, 0, 0))
bpy.data.objects['Cone'].parent = bpy.data.objects['Armature']
bpy.data.objects['Armature'].pose.bones['Bone'].rotation_quaternion = (0.95, 0.1, 0.2, 0.0)
bpy.data.objects['Armature'].pose.bones['Upper'].rotation_mode = 'XYZ'
bpy.data.objects['Armature'].pose.bones['Upper'].rotation_euler = (0.4, 0.0, -0.3)
bpy.data.objects['Armature'].pose.bones['Side'].location = (0.1, 0.2, 0.0)
bpy.data.objects['Armature'].pose.bones['Side'].rotation_quaternion = (0.9, 0.0, 0.3, 0.2)
bpy.data.objects['Armature'].pose.bones['Tip'].scale = (1.0, 1.5, 0.8)
bpy.ops.export_scene.gltf(filepath='out/default.glb')
bpy.ops.export_scene.gltf(filepath='out/def_bones.glb', export_def_bones=True)
bpy.ops.export_scene.gltf(filepath='out/flatten_bones.glb', export_hierarchy_flatten_bones=True)
bpy.ops.export_scene.gltf(filepath='out/leaf.glb', export_leaf_bone=True)
bpy.ops.export_scene.gltf(filepath='out/pose.glb', export_rest_position_armature=False)
bpy.ops.export_scene.gltf(filepath='out/influence2.glb', export_influence_nb=2)
bpy.ops.export_scene.gltf(filepath='out/all_influences.glb', export_all_influences=True)
bpy.ops.export_scene.gltf(filepath='out/no_skins.glb', export_skins=False)
bpy.ops.export_scene.gltf(filepath='out/zup.glb', export_yup=False)
bpy.ops.export_scene.gltf(filepath='out/flatten_objs.glb', export_hierarchy_flatten_objs=True)
bpy.ops.export_scene.gltf(filepath='out/remove_refused.glb', export_armature_object_remove=True)
