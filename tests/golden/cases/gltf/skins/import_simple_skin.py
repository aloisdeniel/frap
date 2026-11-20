# WP 6.8: import of the Khronos sample SimpleSkin (CC0-1.0): unnamed nodes, two joints at the
# root (the dummy root becomes the 'Armature' object), the skinned mesh moved under it. Its
# animation is the animation importer's (WP 6.7): only the rest pose, the bind-pose mesh,
# vertex groups and objects are compared.
# golden: startup=empty order=canonical tier=T1 tol=1e-5 compare=objects,mesh:Mesh_0
import bpy
bpy.ops.import_scene.gltf(filepath='SimpleSkin.gltf')
bpy.data.objects.new('R_Node_1', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['R_Node_1'])
bpy.data.objects['R_Node_1'].matrix_world = bpy.data.objects['Armature'].data.bones['Node_1'].matrix_local
bpy.data.objects['R_Node_1'].scale.x = bpy.data.objects['Armature'].data.bones['Node_1'].length
bpy.data.objects['Armature'].data.bones['Node_1'].parent.name
bpy.data.objects['Armature'].pose.bones['Node_1'].custom_shape.name
bpy.data.objects.new('R_Node_2', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['R_Node_2'])
bpy.data.objects['R_Node_2'].matrix_world = bpy.data.objects['Armature'].data.bones['Node_2'].matrix_local
bpy.data.objects['R_Node_2'].scale.x = bpy.data.objects['Armature'].data.bones['Node_2'].length
bpy.data.objects['Armature'].data.bones['Node_2'].parent.name
bpy.data.objects['Armature'].pose.bones['Node_2'].custom_shape.name
bpy.data.objects.new('W_Node_1', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['W_Node_1'])
bpy.data.objects['W_Node_1'].location.x = bpy.data.objects['Mesh_0'].vertex_groups['Node_1'].weight(0)
bpy.data.objects['W_Node_1'].location.y = bpy.data.objects['Mesh_0'].vertex_groups['Node_1'].weight(4)
bpy.data.objects['W_Node_1'].location.z = bpy.data.objects['Mesh_0'].vertex_groups['Node_1'].weight(8)
bpy.data.objects.new('W_Node_2', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['W_Node_2'])
bpy.data.objects['W_Node_2'].location.x = bpy.data.objects['Mesh_0'].vertex_groups['Node_2'].weight(0)
bpy.data.objects['W_Node_2'].location.y = bpy.data.objects['Mesh_0'].vertex_groups['Node_2'].weight(4)
bpy.data.objects['W_Node_2'].location.z = bpy.data.objects['Mesh_0'].vertex_groups['Node_2'].weight(8)
