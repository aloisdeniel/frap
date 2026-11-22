# WP 8.12: `KHR_animation_pointer` channels of an anisotropy baked from grayscale textures: the
# strength factor (the Multiply's constant in front of the texture on `Anisotropic`) and the
# rotation factor (the unlinked `Anisotropic Rotation`) are animated, so both pointers
# (`anisotropyStrength`, `anisotropyRotation`) are sampled.
# golden: startup=factory tier=T2 compare=glb:out/ptr.glb glb_structure
import bpy
bpy.context.scene.frame_end = 10
bpy.data.images.new('Strength', 2, 2)
bpy.data.images['Strength'].pixels = [0.2, 0.2, 0.2, 1.0, 0.4, 0.5, 0.6, 1.0, 0.8, 0.8, 0.8, 1.0, 1.0, 1.0, 1.0, 1.0]
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Material'].node_tree.nodes['Image Texture'].image = bpy.data.images['Strength']
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeMath')
bpy.data.materials['Material'].node_tree.nodes['Math'].operation = 'MULTIPLY'
bpy.data.materials['Material'].node_tree.nodes['Math'].inputs[1].default_value = 0.5
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Math'].inputs[0])
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Math'].outputs[0], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Anisotropic'])
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Anisotropic Rotation'].default_value = 0.25
bpy.data.materials['Material'].node_tree.nodes['Math'].inputs[1].keyframe_insert('default_value', frame=1)
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Anisotropic Rotation'].keyframe_insert('default_value', frame=1)
bpy.data.materials['Material'].node_tree.nodes['Math'].inputs[1].default_value = 0.9
bpy.data.materials['Material'].node_tree.nodes['Math'].inputs[1].keyframe_insert('default_value', frame=10)
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Anisotropic Rotation'].default_value = 0.5
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Anisotropic Rotation'].keyframe_insert('default_value', frame=10)
bpy.ops.export_scene.gltf(filepath='out/ptr.glb', export_animation_mode='SCENE', export_pointer_animation=True)
