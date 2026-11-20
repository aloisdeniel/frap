# WP 6.8: import of Blender's export of skin_morph (two skinned meshes with shape keys under
# one armature): the shape keys are moved into the bind pose with the mesh (absolute key
# positions skinned like the vertices); key values; vertex groups.
# golden: startup=empty order=canonical tier=T1 tol=1e-5 compare=objects,context,mesh:Cylinder,mesh:Cube.001
import bpy
bpy.ops.import_scene.gltf(filepath='skin_morph.glb')
bpy.data.objects.new('SK', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['SK'])
bpy.data.objects['SK'].location = bpy.data.meshes['Cylinder'].shape_keys.key_blocks['Bulge'].data[1].co
bpy.data.objects['SK'].rotation_euler = bpy.data.meshes['Cylinder'].shape_keys.key_blocks['Bulge'].data[3].co
bpy.data.objects['SK'].scale = (bpy.data.meshes['Cylinder'].shape_keys.key_blocks['Bulge'].value, bpy.data.meshes['Cube.001'].shape_keys.key_blocks['Lift'].value, 1.0)
bpy.data.objects.new('SK2', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['SK2'])
bpy.data.objects['SK2'].location = bpy.data.meshes['Cube.001'].shape_keys.key_blocks['Lift'].data[0].co
bpy.data.objects.new('R_Bone', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['R_Bone'])
bpy.data.objects.new('P_Bone', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P_Bone'])
bpy.data.objects['R_Bone'].matrix_world = bpy.data.objects['Armature'].data.bones['Bone'].matrix_local
bpy.data.objects['R_Bone'].scale.x = bpy.data.objects['Armature'].data.bones['Bone'].length
bpy.data.objects['P_Bone'].matrix_world = bpy.data.objects['Armature'].pose.bones['Bone'].matrix_basis
bpy.data.objects['Armature'].data.bones['Bone'].parent.name
bpy.data.objects['Armature'].pose.bones['Bone'].custom_shape.name
bpy.data.objects.new('R_Top', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['R_Top'])
bpy.data.objects.new('P_Top', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P_Top'])
bpy.data.objects['R_Top'].matrix_world = bpy.data.objects['Armature'].data.bones['Top'].matrix_local
bpy.data.objects['R_Top'].scale.x = bpy.data.objects['Armature'].data.bones['Top'].length
bpy.data.objects['P_Top'].matrix_world = bpy.data.objects['Armature'].pose.bones['Top'].matrix_basis
bpy.data.objects['Armature'].data.bones['Top'].parent.name
bpy.data.objects['Armature'].pose.bones['Top'].custom_shape.name
bpy.data.objects.new('W_Bone', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['W_Bone'])
bpy.data.objects['W_Bone'].location.x = bpy.data.objects['Cylinder'].vertex_groups['Bone'].weight(0)
bpy.data.objects['W_Bone'].location.y = bpy.data.objects['Cylinder'].vertex_groups['Bone'].weight(1)
bpy.data.objects['W_Bone'].location.z = bpy.data.objects['Cylinder'].vertex_groups['Bone'].weight(3)
bpy.data.objects.new('W_Top', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['W_Top'])
bpy.data.objects['W_Top'].location.x = bpy.data.objects['Cylinder'].vertex_groups['Top'].weight(0)
bpy.data.objects['W_Top'].location.y = bpy.data.objects['Cylinder'].vertex_groups['Top'].weight(1)
bpy.data.objects['W_Top'].location.z = bpy.data.objects['Cylinder'].vertex_groups['Top'].weight(3)
