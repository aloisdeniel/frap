# WP 6.4 (blender-rna-datamodel §0.4): the action a key lands in. Shape-key values go into the
# object's action under a `KE` slot (`find_related_ids`: Key → mesh → object); a material's
# node-tree sockets get their own action named after the material with slot
# `NTShader Nodetree`; a camera's data keyed first creates `CameraAction` (slot `CACamera`),
# which the camera object then shares under `OBCamera`; a second object assigned an existing
# action gets its own slot when keyed. The pose-bone case lives in `pose_bones.py`.
# golden: startup=factory tier=T1 tol=1e-5 compare=anim:CubeAction,anim:MaterialAction,anim:CameraAction,objects
import bpy
bpy.data.objects['Cube'].keyframe_insert('location', frame=1)
bpy.ops.object.shape_key_add(from_mix=False)
bpy.ops.object.shape_key_add(from_mix=False)
bpy.data.shape_keys['Key'].key_blocks['Key 1'].value = 0.25
bpy.data.shape_keys['Key'].key_blocks['Key 1'].keyframe_insert('value', frame=2)
bpy.data.shape_keys['Key'].key_blocks['Key 1'].value = 1.0
bpy.data.shape_keys['Key'].key_blocks['Key 1'].keyframe_insert('value', frame=12)
bpy.data.shape_keys['Key'].key_blocks['Key 1'].keyframe_insert('slider_max', frame=12)
bpy.data.actions['CubeAction'].slots['KEKey']
bpy.data.shape_keys['Key'].animation_data.action_slot.identifier
bpy.data.objects.new('ke_' + bpy.data.shape_keys['Key'].animation_data.action.name, None)
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.2
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].keyframe_insert('default_value', frame=1)
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.9
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].keyframe_insert('default_value', frame=9)
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].keyframe_insert('default_value', frame=4)
bpy.data.materials['Material'].node_tree.keyframe_insert('nodes["Principled BSDF"].inputs[1].default_value', frame=6)
bpy.data.actions['MaterialAction'].slots['NTShader Nodetree']
bpy.data.objects.new('nt_' + bpy.data.materials['Material'].node_tree.animation_data.action.name, None)
bpy.data.materials['Material'].keyframe_insert('metallic', frame=3)
bpy.data.objects.new('ma_' + bpy.data.materials['Material'].animation_data.action.name, None)
bpy.data.cameras['Camera'].keyframe_insert('lens', frame=1)
bpy.data.cameras['Camera'].lens = 85.0
bpy.data.cameras['Camera'].keyframe_insert('lens', frame=10)
bpy.data.objects['Camera'].keyframe_insert('rotation_euler', frame=1)
bpy.data.actions['CameraAction'].slots['CACamera']
bpy.data.actions['CameraAction'].slots['OBCamera']
bpy.data.objects['Light'].animation_data_create()
bpy.data.objects['Light'].animation_data.action = bpy.data.actions['CubeAction']
bpy.data.objects['Light'].scale.x = bpy.data.objects['Light'].animation_data.action_slot_handle + 2
bpy.data.objects['Light'].keyframe_insert('location', frame=5)
bpy.data.actions['CubeAction'].slots['OBLight']
bpy.context.scene.frame_set(7)
bpy.data.objects['Cube'].location = (bpy.data.shape_keys['Key'].key_blocks['Key 1'].value, bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value, bpy.data.cameras['Camera'].lens)
