# M5+M6 W3 integration: KHR_animation_pointer channels into node TRS and morph weights
# (`/nodes/1/translation|rotation`, `/meshes/0/weights` broadcast to the node instancing the
# mesh) join the core node channels (`scale` of the same node) in the node's channel list, and
# a camera pointer of the same animation lands in the same action (slots KETri, OBEmpty,
# CASlot of one action `Move`), as in the add-on's `pre_compute` / `BlenderAnimation.anim`.
# The second animation (`Spin`, a node rotation pointer) stays stashed. Hand-written file:
# import_nodes.inputs/nodes.gltf.
# golden: startup=factory tier=T2 compare=anim:Move,anim:Spin,objects
import bpy
bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath='nodes.gltf')
bpy.context.scene.frame_set(12)
bpy.data.objects['Cam'].location = (bpy.data.cameras['Camera'].lens, bpy.data.shape_keys['TriMesh'].key_blocks['Up'].value, bpy.data.objects['Empty'].animation_data.nla_tracks[0].strips[0].frame_end)
bpy.data.objects['Cam'].name = bpy.data.actions['Move'].slots[0].identifier + '|' + bpy.data.actions['Move'].slots[1].identifier + '|' + bpy.data.actions['Move'].slots[2].identifier + '|' + bpy.data.actions['Spin'].slots[0].identifier
bpy.ops.object.empty_add()
bpy.context.object.name = bpy.data.cameras['Camera'].animation_data.action.name + '|' + bpy.data.cameras['Camera'].animation_data.action_slot.identifier + '|' + bpy.data.shape_keys['TriMesh'].animation_data.action.name + '|' + bpy.data.objects['Tri'].animation_data.nla_tracks[0].name
