# WP 6.8: import of the Khronos sample RiggedSimple (CC-BY-4.0, Cesium; LICENSE.md beside it):
# a Z_UP root, an Armature node whose joints come with a skeleton, a skinned cylinder under it.
# Its animation is the animation importer's (WP 6.7): only the rest pose, the bind-pose mesh,
# vertex groups and objects are compared.
# golden: startup=empty order=canonical tier=T1 tol=1e-5 compare=objects,mesh:Cylinder
import bpy
bpy.ops.import_scene.gltf(filepath='RiggedSimple.glb')
bpy.data.objects.new('R_Bone', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['R_Bone'])
bpy.data.objects['R_Bone'].matrix_world = bpy.data.objects['Armature'].data.bones['Bone'].matrix_local
bpy.data.objects['R_Bone'].scale.x = bpy.data.objects['Armature'].data.bones['Bone'].length
bpy.data.objects['Armature'].data.bones['Bone'].parent.name
bpy.data.objects['Armature'].pose.bones['Bone'].custom_shape.name
bpy.data.objects.new('R_Bone.001', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['R_Bone.001'])
bpy.data.objects['R_Bone.001'].matrix_world = bpy.data.objects['Armature'].data.bones['Bone.001'].matrix_local
bpy.data.objects['R_Bone.001'].scale.x = bpy.data.objects['Armature'].data.bones['Bone.001'].length
bpy.data.objects['Armature'].data.bones['Bone.001'].parent.name
bpy.data.objects['Armature'].pose.bones['Bone.001'].custom_shape.name
bpy.data.objects.new('W_Bone', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['W_Bone'])
bpy.data.objects['W_Bone'].location.x = bpy.data.objects['Cylinder'].vertex_groups['Bone'].weight(0)
bpy.data.objects['W_Bone'].location.y = bpy.data.objects['Cylinder'].vertex_groups['Bone'].weight(10)
bpy.data.objects['W_Bone'].location.z = bpy.data.objects['Cylinder'].vertex_groups['Bone'].weight(20)
bpy.data.objects.new('W_Bone.001', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['W_Bone.001'])
bpy.data.objects['W_Bone.001'].location.x = bpy.data.objects['Cylinder'].vertex_groups['Bone.001'].weight(0)
bpy.data.objects['W_Bone.001'].location.y = bpy.data.objects['Cylinder'].vertex_groups['Bone.001'].weight(10)
bpy.data.objects['W_Bone.001'].location.z = bpy.data.objects['Cylinder'].vertex_groups['Bone.001'].weight(20)
