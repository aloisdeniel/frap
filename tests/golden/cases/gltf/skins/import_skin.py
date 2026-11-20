# WP 6.8: import of a skinned rig (Blender's export of skin_options: a connected chain, a
# branch, non-deforming bones, an object parented to a bone, a child of the skinned mesh, a
# plain child of the armature). The default bone_heuristic BLENDER: bones along their joint's
# +Y turned to Blender's +Y, lengths from the children, the icosphere bone shape in the hidden
# glTF_not_exported collection; guess_original_bind_pose from the inverse bind matrices; the
# skinned mesh in the bind pose with an ARMATURE modifier and one vertex group per joint; the
# bone-parented object (parent_type BONE). Rest matrices, lengths, parents, pose and weights
# are copied into empties to be compared.
# golden: startup=empty order=canonical tier=T1 tol=1e-5 compare=objects,context,mesh:Sphere
import bpy
bpy.ops.import_scene.gltf(filepath='skin.glb')
bpy.data.objects.new('R_Bone', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['R_Bone'])
bpy.data.objects.new('P_Bone', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P_Bone'])
bpy.data.objects['R_Bone'].matrix_world = bpy.data.objects['Armature'].data.bones['Bone'].matrix_local
bpy.data.objects['R_Bone'].scale.x = bpy.data.objects['Armature'].data.bones['Bone'].length
bpy.data.objects['P_Bone'].matrix_world = bpy.data.objects['Armature'].pose.bones['Bone'].matrix_basis
bpy.data.objects['Armature'].data.bones['Bone'].parent.name
bpy.data.objects['Armature'].pose.bones['Bone'].custom_shape.name
bpy.data.objects.new('R_Side', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['R_Side'])
bpy.data.objects.new('P_Side', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P_Side'])
bpy.data.objects['R_Side'].matrix_world = bpy.data.objects['Armature'].data.bones['Side'].matrix_local
bpy.data.objects['R_Side'].scale.x = bpy.data.objects['Armature'].data.bones['Side'].length
bpy.data.objects['P_Side'].matrix_world = bpy.data.objects['Armature'].pose.bones['Side'].matrix_basis
bpy.data.objects['Armature'].data.bones['Side'].parent.name
bpy.data.objects['Armature'].pose.bones['Side'].custom_shape.name
bpy.data.objects.new('R_Upper', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['R_Upper'])
bpy.data.objects.new('P_Upper', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P_Upper'])
bpy.data.objects['R_Upper'].matrix_world = bpy.data.objects['Armature'].data.bones['Upper'].matrix_local
bpy.data.objects['R_Upper'].scale.x = bpy.data.objects['Armature'].data.bones['Upper'].length
bpy.data.objects['P_Upper'].matrix_world = bpy.data.objects['Armature'].pose.bones['Upper'].matrix_basis
bpy.data.objects['Armature'].data.bones['Upper'].parent.name
bpy.data.objects['Armature'].pose.bones['Upper'].custom_shape.name
bpy.data.objects.new('R_Helper', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['R_Helper'])
bpy.data.objects.new('P_Helper', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P_Helper'])
bpy.data.objects['R_Helper'].matrix_world = bpy.data.objects['Armature'].data.bones['Helper'].matrix_local
bpy.data.objects['R_Helper'].scale.x = bpy.data.objects['Armature'].data.bones['Helper'].length
bpy.data.objects['P_Helper'].matrix_world = bpy.data.objects['Armature'].pose.bones['Helper'].matrix_basis
bpy.data.objects['Armature'].data.bones['Helper'].parent.name
bpy.data.objects['Armature'].pose.bones['Helper'].custom_shape.name
bpy.data.objects.new('R_Tip', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['R_Tip'])
bpy.data.objects.new('P_Tip', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P_Tip'])
bpy.data.objects['R_Tip'].matrix_world = bpy.data.objects['Armature'].data.bones['Tip'].matrix_local
bpy.data.objects['R_Tip'].scale.x = bpy.data.objects['Armature'].data.bones['Tip'].length
bpy.data.objects['P_Tip'].matrix_world = bpy.data.objects['Armature'].pose.bones['Tip'].matrix_basis
bpy.data.objects['Armature'].data.bones['Tip'].parent.name
bpy.data.objects['Armature'].pose.bones['Tip'].custom_shape.name
bpy.data.objects.new('W_Bone', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['W_Bone'])
bpy.data.objects['W_Bone'].location.x = bpy.data.objects['Sphere'].vertex_groups['Bone'].weight(3)
bpy.data.objects['W_Bone'].location.y = bpy.data.objects['Sphere'].vertex_groups['Bone'].weight(20)
bpy.data.objects['W_Bone'].location.z = bpy.data.objects['Sphere'].vertex_groups['Bone'].weight(30)
bpy.data.objects.new('W_Upper', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['W_Upper'])
bpy.data.objects['W_Upper'].location.x = bpy.data.objects['Sphere'].vertex_groups['Upper'].weight(3)
bpy.data.objects['W_Upper'].location.y = bpy.data.objects['Sphere'].vertex_groups['Upper'].weight(20)
bpy.data.objects['W_Upper'].location.z = bpy.data.objects['Sphere'].vertex_groups['Upper'].weight(30)
bpy.data.objects.new('W_Side', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['W_Side'])
bpy.data.objects['W_Side'].location.x = bpy.data.objects['Sphere'].vertex_groups['Side'].weight(3)
bpy.data.objects['W_Side'].location.y = bpy.data.objects['Sphere'].vertex_groups['Side'].weight(20)
bpy.data.objects['W_Side'].location.z = bpy.data.objects['Sphere'].vertex_groups['Side'].weight(30)
bpy.data.objects.new('W_Helper', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['W_Helper'])
bpy.data.objects['W_Helper'].location.x = bpy.data.objects['Sphere'].vertex_groups['Helper'].weight(3)
bpy.data.objects['W_Helper'].location.y = bpy.data.objects['Sphere'].vertex_groups['Helper'].weight(20)
bpy.data.objects['W_Helper'].location.z = bpy.data.objects['Sphere'].vertex_groups['Helper'].weight(30)
bpy.data.objects.new('W_Tip', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['W_Tip'])
bpy.data.objects['W_Tip'].location.x = bpy.data.objects['Sphere'].vertex_groups['Tip'].weight(3)
bpy.data.objects['W_Tip'].location.y = bpy.data.objects['Sphere'].vertex_groups['Tip'].weight(20)
bpy.data.objects['W_Tip'].location.z = bpy.data.objects['Sphere'].vertex_groups['Tip'].weight(30)
