# WP 6.8: import of a hand-written skin (`import_rig.inputs/rig.gltf`, made by a script): the
# joints' deepest common ancestor is the scene root, so the dummy root becomes the armature
# object (named after the skin); a joint with a matrix, a non-joint mesh child of a joint (parented
# to the bone, moved back from its tip), a joint carrying a mesh (moved to a child object), a
# camera under a joint; a skinned node that can move under the armature and one that cannot (it
# has a child: its mesh goes to a new '.skinned' object); normalized UNSIGNED_BYTE weights, a
# second joint set, a vertex without weight (given to its first joint); inverse bind matrices of a
# rest pose that differs from the node TRS (guess_original_bind_pose).
# golden: startup=empty order=canonical tier=T1 tol=1e-5 compare=objects,context,mesh:BodyMesh
import bpy
bpy.ops.import_scene.gltf(filepath='rig.gltf')
bpy.data.objects.new('R_Root', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['R_Root'])
bpy.data.objects.new('P_Root', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P_Root'])
bpy.data.objects['R_Root'].matrix_world = bpy.data.objects['Rig'].data.bones['Root'].matrix_local
bpy.data.objects['R_Root'].scale.x = bpy.data.objects['Rig'].data.bones['Root'].length
bpy.data.objects['P_Root'].matrix_world = bpy.data.objects['Rig'].pose.bones['Root'].matrix_basis
bpy.data.objects['Rig'].data.bones['Root'].parent.name
bpy.data.objects['Rig'].pose.bones['Root'].custom_shape.name
bpy.data.objects.new('R_Spine', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['R_Spine'])
bpy.data.objects.new('P_Spine', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P_Spine'])
bpy.data.objects['R_Spine'].matrix_world = bpy.data.objects['Rig'].data.bones['Spine'].matrix_local
bpy.data.objects['R_Spine'].scale.x = bpy.data.objects['Rig'].data.bones['Spine'].length
bpy.data.objects['P_Spine'].matrix_world = bpy.data.objects['Rig'].pose.bones['Spine'].matrix_basis
bpy.data.objects['Rig'].data.bones['Spine'].parent.name
bpy.data.objects['Rig'].pose.bones['Spine'].custom_shape.name
bpy.data.objects.new('R_Head', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['R_Head'])
bpy.data.objects.new('P_Head', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P_Head'])
bpy.data.objects['R_Head'].matrix_world = bpy.data.objects['Rig'].data.bones['Head'].matrix_local
bpy.data.objects['R_Head'].scale.x = bpy.data.objects['Rig'].data.bones['Head'].length
bpy.data.objects['P_Head'].matrix_world = bpy.data.objects['Rig'].pose.bones['Head'].matrix_basis
bpy.data.objects['Rig'].data.bones['Head'].parent.name
bpy.data.objects['Rig'].pose.bones['Head'].custom_shape.name
bpy.data.objects.new('R_Arm', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['R_Arm'])
bpy.data.objects.new('P_Arm', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P_Arm'])
bpy.data.objects['R_Arm'].matrix_world = bpy.data.objects['Rig'].data.bones['Arm'].matrix_local
bpy.data.objects['R_Arm'].scale.x = bpy.data.objects['Rig'].data.bones['Arm'].length
bpy.data.objects['P_Arm'].matrix_world = bpy.data.objects['Rig'].pose.bones['Arm'].matrix_basis
bpy.data.objects['Rig'].data.bones['Arm'].parent.name
bpy.data.objects['Rig'].pose.bones['Arm'].custom_shape.name
bpy.data.objects.new('W_Root', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['W_Root'])
bpy.data.objects['W_Root'].location.x = bpy.data.objects['Body'].vertex_groups['Root'].weight(0)
bpy.data.objects['W_Root'].location.y = bpy.data.objects['Body'].vertex_groups['Root'].weight(4)
bpy.data.objects['W_Root'].location.z = bpy.data.objects['Body'].vertex_groups['Root'].weight(5)
bpy.data.objects.new('W_Spine', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['W_Spine'])
bpy.data.objects['W_Spine'].location.x = bpy.data.objects['Body'].vertex_groups['Spine'].weight(0)
bpy.data.objects['W_Spine'].location.y = bpy.data.objects['Body'].vertex_groups['Spine'].weight(4)
bpy.data.objects['W_Spine'].location.z = bpy.data.objects['Body'].vertex_groups['Spine'].weight(5)
bpy.data.objects.new('W_Head', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['W_Head'])
bpy.data.objects['W_Head'].location.x = bpy.data.objects['Body'].vertex_groups['Head'].weight(0)
bpy.data.objects['W_Head'].location.y = bpy.data.objects['Body'].vertex_groups['Head'].weight(4)
bpy.data.objects['W_Head'].location.z = bpy.data.objects['Body'].vertex_groups['Head'].weight(5)
bpy.data.objects.new('W_Arm', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['W_Arm'])
bpy.data.objects['W_Arm'].location.x = bpy.data.objects['Body'].vertex_groups['Arm'].weight(0)
bpy.data.objects['W_Arm'].location.y = bpy.data.objects['Body'].vertex_groups['Arm'].weight(4)
bpy.data.objects['W_Arm'].location.z = bpy.data.objects['Body'].vertex_groups['Arm'].weight(5)
