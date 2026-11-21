# WP 3.6: import raise round-trip of extension textures (the add-on's
# scalar_factor_and_texture, color_factor_and_texture and normal_map): a transmission texture
# with a 0.5 factor (Math MULTIPLY <- Separate Color R <- "TRANSMISSION"), a clearcoat normal
# map (Normal Map <- "CLEARCOAT NORMAL"), a specular colour texture and a sheen colour texture
# with a factor ("Sheen Color Factor" Mix). Node names follow the importer's creation order.
# golden: startup=factory tier=T2 compare=materials,glb:out/again.glb glb_structure
import bpy
bpy.data.images.new('Frost', 4, 4)
bpy.data.images.new('Bumps', 4, 4)
bpy.data.images.new('Tint', 4, 4)
bpy.data.images.new('Fibres', 4, 4)
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Material'].node_tree.nodes['Image Texture'].image = bpy.data.images['Frost']
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeMath')
bpy.data.materials['Material'].node_tree.nodes['Math'].operation = 'MULTIPLY'
bpy.data.materials['Material'].node_tree.nodes['Math'].inputs[1].default_value = 0.5
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Math'].inputs[0])
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Math'].outputs[0], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Transmission Weight'])
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Coat Weight'].default_value = 0.8
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Material'].node_tree.nodes['Image Texture.001'].image = bpy.data.images['Bumps']
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeNormalMap')
bpy.data.materials['Material'].node_tree.nodes['Normal Map'].inputs['Strength'].default_value = 0.5
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Image Texture.001'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Normal Map'].inputs['Color'])
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Normal Map'].outputs['Normal'], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Coat Normal'])
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Material'].node_tree.nodes['Image Texture.002'].image = bpy.data.images['Tint']
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Image Texture.002'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Specular Tint'])
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Sheen Weight'].default_value = 1.0
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Material'].node_tree.nodes['Image Texture.003'].image = bpy.data.images['Fibres']
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeMix')
bpy.data.materials['Material'].node_tree.nodes['Mix'].data_type = 'RGBA'
bpy.data.materials['Material'].node_tree.nodes['Mix'].blend_type = 'MULTIPLY'
bpy.data.materials['Material'].node_tree.nodes['Mix'].inputs['Factor'].default_value = 1.0
bpy.data.materials['Material'].node_tree.nodes['Mix'].inputs[7].default_value = (0.5, 0.25, 1.0, 1)
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Image Texture.003'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Mix'].inputs[6])
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Mix'].outputs[2], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Sheen Tint'])
bpy.ops.export_scene.gltf(filepath='out/first.glb')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/first.glb')
bpy.ops.export_scene.gltf(filepath='out/again.glb')
