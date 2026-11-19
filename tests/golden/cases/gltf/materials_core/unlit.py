# WP 3.5: KHR_materials_unlit. A Background shader on the output; the importer's "lightpath
# trick" (Light Path `Is Camera Ray` mixing a Transparent BSDF and an Emission) behind a Mix
# Shader with a Transparent BSDF whose factor (0.5) is the alpha → BLEND; an RGB node straight
# into Surface. Each gives the extension, the unclamped colour as baseColorFactor, metallic 0
# and roughness 0.9. An Emission shader alone is not unlit: it exports emissive (colour × 5 →
# KHR_materials_emissive_strength) with the add-on's black base colour.
# golden: startup=factory tier=T2 compare=glb:out/unlit.glb
import bpy
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeBackground')
bpy.data.materials['Material'].node_tree.nodes['Background'].inputs['Color'].default_value = (0.2, 0.4, 0.6, 1.0)
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Background'].outputs[0], bpy.data.materials['Material'].node_tree.nodes['Material Output'].inputs['Surface'])
bpy.data.meshes['Cube'].materials.append(bpy.data.materials.new('Trick'))
bpy.data.materials['Trick'].node_tree.nodes.new('ShaderNodeEmission')
bpy.data.materials['Trick'].node_tree.nodes['Emission'].inputs['Color'].default_value = (1.5, 0.1, 0.1, 1.0)
bpy.data.materials['Trick'].node_tree.nodes.new('ShaderNodeLightPath')
bpy.data.materials['Trick'].node_tree.nodes.new('ShaderNodeBsdfTransparent')
bpy.data.materials['Trick'].node_tree.nodes.new('ShaderNodeMixShader')
bpy.data.materials['Trick'].node_tree.nodes.new('ShaderNodeMixShader')
bpy.data.materials['Trick'].node_tree.links.new(bpy.data.materials['Trick'].node_tree.nodes['Light Path'].outputs['Is Camera Ray'], bpy.data.materials['Trick'].node_tree.nodes['Mix Shader'].inputs[0])
bpy.data.materials['Trick'].node_tree.links.new(bpy.data.materials['Trick'].node_tree.nodes['Transparent BSDF'].outputs[0], bpy.data.materials['Trick'].node_tree.nodes['Mix Shader'].inputs[1])
bpy.data.materials['Trick'].node_tree.links.new(bpy.data.materials['Trick'].node_tree.nodes['Emission'].outputs[0], bpy.data.materials['Trick'].node_tree.nodes['Mix Shader'].inputs[2])
bpy.data.materials['Trick'].node_tree.nodes['Mix Shader.001'].inputs[0].default_value = 0.5
bpy.data.materials['Trick'].node_tree.links.new(bpy.data.materials['Trick'].node_tree.nodes['Transparent BSDF'].outputs[0], bpy.data.materials['Trick'].node_tree.nodes['Mix Shader.001'].inputs[1])
bpy.data.materials['Trick'].node_tree.links.new(bpy.data.materials['Trick'].node_tree.nodes['Mix Shader'].outputs[0], bpy.data.materials['Trick'].node_tree.nodes['Mix Shader.001'].inputs[2])
bpy.data.materials['Trick'].node_tree.links.new(bpy.data.materials['Trick'].node_tree.nodes['Mix Shader.001'].outputs[0], bpy.data.materials['Trick'].node_tree.nodes['Material Output'].inputs['Surface'])
bpy.data.meshes['Cube'].materials.append(bpy.data.materials.new('Rgb'))
bpy.data.materials['Rgb'].node_tree.nodes.new('ShaderNodeRGB')
bpy.data.materials['Rgb'].node_tree.nodes['Color'].outputs[0].default_value = (1.0, 0.5, 0.0, 1.0)
bpy.data.materials['Rgb'].node_tree.links.new(bpy.data.materials['Rgb'].node_tree.nodes['Color'].outputs[0], bpy.data.materials['Rgb'].node_tree.nodes['Material Output'].inputs['Surface'])
bpy.data.meshes['Cube'].materials.append(bpy.data.materials.new('Glow'))
bpy.data.materials['Glow'].node_tree.nodes.new('ShaderNodeEmission')
bpy.data.materials['Glow'].node_tree.nodes['Emission'].inputs['Color'].default_value = (1.0, 0.5, 0.2, 1.0)
bpy.data.materials['Glow'].node_tree.nodes['Emission'].inputs['Strength'].default_value = 5.0
bpy.data.materials['Glow'].node_tree.links.new(bpy.data.materials['Glow'].node_tree.nodes['Emission'].outputs[0], bpy.data.materials['Glow'].node_tree.nodes['Material Output'].inputs['Surface'])
bpy.data.materials['Glow'].use_backface_culling = True
bpy.data.meshes['Cube'].polygons[1].material_index = 1
bpy.data.meshes['Cube'].polygons[2].material_index = 2
bpy.data.meshes['Cube'].polygons[3].material_index = 3
bpy.ops.export_scene.gltf(filepath='out/unlit.glb')
