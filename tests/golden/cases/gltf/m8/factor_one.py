# WP 8.12: factors of 1 the add-on writes anyway. `occlusionTexture.strength` comes from a Mix
# (MIX) of white and the occlusion texture in front of the glTF Material Output's `Occlusion`:
# a factor of 1 is still written (`"strength": 1`). A `Specular Tint` linked to a Multiply Mix
# of white and a texture has the factor `[1, 1, 1]` (`get_factor_from_socket` returns a list,
# never equal to the tuple `(1.0, 1.0, 1.0)`), so `specularColorFactor` is written as well.
# golden: startup=factory tier=T2 compare=glb:out/one.glb glb_structure
import bpy
bpy.data.node_groups.new('glTF Material Output', 'ShaderNodeTree')
bpy.data.node_groups['glTF Material Output'].interface.new_socket('Occlusion', socket_type='NodeSocketFloat')
bpy.data.node_groups['glTF Material Output'].nodes.new('NodeGroupInput')
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeGroup')
bpy.data.materials['Material'].node_tree.nodes['Group'].node_tree = bpy.data.node_groups['glTF Material Output']
bpy.data.images.new('AO', 4, 4)
bpy.data.images['AO'].generated_color = (0.6, 0.6, 0.6, 1.0)
bpy.data.images.new('Tint', 4, 4)
bpy.data.images['Tint'].generated_color = (0.9, 0.5, 0.2, 1.0)
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Material'].node_tree.nodes['Image Texture'].image = bpy.data.images['AO']
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeMix')
bpy.data.materials['Material'].node_tree.nodes['Mix'].data_type = 'RGBA'
bpy.data.materials['Material'].node_tree.nodes['Mix'].blend_type = 'MIX'
bpy.data.materials['Material'].node_tree.nodes['Mix'].inputs['Factor'].default_value = 1.0
bpy.data.materials['Material'].node_tree.nodes['Mix'].inputs[6].default_value = (1.0, 1.0, 1.0, 1.0)
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Mix'].inputs[7])
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Mix'].outputs[2], bpy.data.materials['Material'].node_tree.nodes['Group'].inputs['Occlusion'])
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Material'].node_tree.nodes['Image Texture.001'].image = bpy.data.images['Tint']
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeMix')
bpy.data.materials['Material'].node_tree.nodes['Mix.001'].data_type = 'RGBA'
bpy.data.materials['Material'].node_tree.nodes['Mix.001'].blend_type = 'MULTIPLY'
bpy.data.materials['Material'].node_tree.nodes['Mix.001'].inputs['Factor'].default_value = 1.0
bpy.data.materials['Material'].node_tree.nodes['Mix.001'].inputs[6].default_value = (1.0, 1.0, 1.0, 1.0)
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Image Texture.001'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Mix.001'].inputs[7])
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Mix.001'].outputs[2], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Specular Tint'])
bpy.ops.export_scene.gltf(filepath='out/one.glb')
