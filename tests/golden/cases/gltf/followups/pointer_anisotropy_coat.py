# WP 7.7: KHR_animation_pointer with the glTF importer's anisotropy texture set-up (detect_anisotropy_nodes:
# Tangent in UV_MAP mode, Image Texture -> Vector Math MULTIPLY_ADD -> Separate XYZ, Z x Math MULTIPLY ->
# Anisotropic, ARCTAN2 -> ADD -> DIVIDE by 2 pi -> Anisotropic Rotation): the MULTIPLY's second input is
# anisotropyStrength, the ADD's second input the raw anisotropyRotation, the texture's Mapping node its
# KHR_texture_transform (Location keyed). Plane Coat: a Normal Map feeding only Coat Normal, its Strength
# keyed: clearcoatNormalTexture/scale alone.
# golden: startup=factory tier=T2 compare=glb:out/aniso.glb glb_structure
import bpy
bpy.context.scene.frame_end = 4
bpy.data.images.new('A', 4, 4)
bpy.data.images['A'].generated_color = (0.8, 0.5, 1.0, 1.0)
bpy.data.images.new('N', 4, 4)
bpy.data.images['N'].generated_color = (0.5, 0.5, 1.0, 1.0)
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTangent')
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeMapping')
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeVectorMath')
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeSeparateXYZ')
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeMath')
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeMath')
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeMath')
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeMath')
bpy.data.materials['Material'].node_tree.nodes['Tangent'].direction_type = 'UV_MAP'
bpy.data.materials['Material'].node_tree.nodes['Image Texture'].image = bpy.data.images['A']
bpy.data.materials['Material'].node_tree.nodes['Vector Math'].operation = 'MULTIPLY_ADD'
bpy.data.materials['Material'].node_tree.nodes['Vector Math'].inputs[1].default_value = (2.0, 2.0, 1.0)
bpy.data.materials['Material'].node_tree.nodes['Vector Math'].inputs[2].default_value = (-1.0, -1.0, 0.0)
bpy.data.materials['Material'].node_tree.nodes['Math'].operation = 'MULTIPLY'
bpy.data.materials['Material'].node_tree.nodes['Math.001'].operation = 'ARCTAN2'
bpy.data.materials['Material'].node_tree.nodes['Math.002'].operation = 'ADD'
bpy.data.materials['Material'].node_tree.nodes['Math.002'].inputs[1].default_value = 0.25
bpy.data.materials['Material'].node_tree.nodes['Math.003'].operation = 'DIVIDE'
bpy.data.materials['Material'].node_tree.nodes['Math.003'].inputs[1].default_value = 6.283185
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Tangent'].outputs['Tangent'], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Tangent'])
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Mapping'].outputs['Vector'], bpy.data.materials['Material'].node_tree.nodes['Image Texture'].inputs['Vector'])
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Vector Math'].inputs[0])
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Vector Math'].outputs[0], bpy.data.materials['Material'].node_tree.nodes['Separate XYZ'].inputs[0])
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Separate XYZ'].outputs['Z'], bpy.data.materials['Material'].node_tree.nodes['Math'].inputs[0])
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Math'].outputs[0], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Anisotropic'])
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Separate XYZ'].outputs['Y'], bpy.data.materials['Material'].node_tree.nodes['Math.001'].inputs[0])
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Separate XYZ'].outputs['X'], bpy.data.materials['Material'].node_tree.nodes['Math.001'].inputs[1])
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Math.001'].outputs[0], bpy.data.materials['Material'].node_tree.nodes['Math.002'].inputs[0])
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Math.002'].outputs[0], bpy.data.materials['Material'].node_tree.nodes['Math.003'].inputs[0])
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Math.003'].outputs[0], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Anisotropic Rotation'])
bpy.data.materials['Material'].node_tree.nodes['Math'].inputs[1].default_value = 0.2
bpy.data.materials['Material'].node_tree.nodes['Math'].inputs[1].keyframe_insert('default_value', frame=1)
bpy.data.materials['Material'].node_tree.nodes['Math'].inputs[1].default_value = 0.9
bpy.data.materials['Material'].node_tree.nodes['Math'].inputs[1].keyframe_insert('default_value', frame=4)
bpy.data.materials['Material'].node_tree.nodes['Mapping'].inputs['Location'].keyframe_insert('default_value', frame=1)
bpy.data.materials['Material'].node_tree.nodes['Mapping'].inputs['Location'].default_value = (0.0, 0.5, 0.0)
bpy.data.materials['Material'].node_tree.nodes['Mapping'].inputs['Location'].keyframe_insert('default_value', frame=4)
bpy.ops.mesh.primitive_plane_add(location=(3, 0, 0))
bpy.context.object.data.materials.append(bpy.data.materials.new('Coat'))
bpy.data.materials['Coat'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Coat'].node_tree.nodes['Image Texture'].image = bpy.data.images['N']
bpy.data.materials['Coat'].node_tree.nodes.new('ShaderNodeNormalMap')
bpy.data.materials['Coat'].node_tree.links.new(bpy.data.materials['Coat'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Coat'].node_tree.nodes['Normal Map'].inputs['Color'])
bpy.data.materials['Coat'].node_tree.links.new(bpy.data.materials['Coat'].node_tree.nodes['Normal Map'].outputs['Normal'], bpy.data.materials['Coat'].node_tree.nodes['Principled BSDF'].inputs['Coat Normal'])
bpy.data.materials['Coat'].node_tree.nodes['Principled BSDF'].inputs['Coat Weight'].default_value = 0.5
bpy.data.materials['Coat'].node_tree.nodes['Normal Map'].inputs['Strength'].default_value = 0.25
bpy.data.materials['Coat'].node_tree.nodes['Normal Map'].inputs['Strength'].keyframe_insert('default_value', frame=1)
bpy.data.materials['Coat'].node_tree.nodes['Normal Map'].inputs['Strength'].default_value = 1.5
bpy.data.materials['Coat'].node_tree.nodes['Normal Map'].inputs['Strength'].keyframe_insert('default_value', frame=4)
bpy.context.scene.frame_set(1)
bpy.ops.export_scene.gltf(filepath='out/aniso.glb', export_animation_mode='SCENE', export_pointer_animation=True)
