# WP 3.6: KHR_materials_sheen export. Constant tint and roughness on the first material; a
# sheen colour texture (copied whole) behind a Mix MULTIPLY factor on the second; no extension
# for a material whose Sheen Weight is 0 (the tint is ignored).
# golden: startup=factory tier=T2 compare=glb:out/sheen.glb
import bpy
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Sheen Weight'].default_value = 1.0
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Sheen Tint'].default_value = (0.2, 0.3, 0.4, 1)
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Sheen Roughness'].default_value = 0.25
bpy.data.materials.new('Velvet')
bpy.data.materials['Velvet'].node_tree.nodes['Principled BSDF'].inputs['Sheen Weight'].default_value = 0.5
bpy.data.images.new('Fibres', 4, 4)
bpy.data.materials['Velvet'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Velvet'].node_tree.nodes['Image Texture'].image = bpy.data.images['Fibres']
bpy.data.materials['Velvet'].node_tree.nodes.new('ShaderNodeMix')
bpy.data.materials['Velvet'].node_tree.nodes['Mix'].data_type = 'RGBA'
bpy.data.materials['Velvet'].node_tree.nodes['Mix'].blend_type = 'MULTIPLY'
bpy.data.materials['Velvet'].node_tree.nodes['Mix'].inputs['Factor'].default_value = 1.0
bpy.data.materials['Velvet'].node_tree.nodes['Mix'].inputs[7].default_value = (0.5, 0.25, 1.0, 1)
bpy.data.materials['Velvet'].node_tree.links.new(bpy.data.materials['Velvet'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Velvet'].node_tree.nodes['Mix'].inputs[6])
bpy.data.materials['Velvet'].node_tree.links.new(bpy.data.materials['Velvet'].node_tree.nodes['Mix'].outputs[2], bpy.data.materials['Velvet'].node_tree.nodes['Principled BSDF'].inputs['Sheen Tint'])
bpy.data.materials.new('Plain')
bpy.data.materials['Plain'].node_tree.nodes['Principled BSDF'].inputs['Sheen Tint'].default_value = (1, 0, 0, 1)
bpy.ops.mesh.primitive_plane_add(location=(3, 0, 0))
bpy.context.object.data.materials.append(bpy.data.materials['Velvet'])
bpy.ops.mesh.primitive_plane_add(location=(-3, 0, 0))
bpy.context.object.data.materials.append(bpy.data.materials['Plain'])
bpy.ops.export_scene.gltf(filepath='out/sheen.glb')
