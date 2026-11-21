# WP 7.7: export_image_format='NONE' reads the base colour and the emission from the sockets'
# own values (get_const_from_default_value_socket), linked or not: Base Color fed by an Image
# Texture keeps its own (0.8, 0.8, 0.8) as baseColorFactor, Emission Color fed by the same
# texture its (0.2, 0.5, 1.0) at strength 2; a second material reads a colour attribute into
# Base Color: with NONE the colour attribute is not gathered (no COLOR_0 from it) and the
# socket's own (0.1, 0.2, 0.3) is the factor. The default export writes the textures.
# golden: startup=factory tier=T2 compare=glb:out/none.glb,glb:out/auto.glb glb_structure
import bpy
bpy.data.images.new('T', 4, 4)
bpy.data.images['T'].generated_color = (0.9, 0.6, 0.3, 1.0)
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Material'].node_tree.nodes['Image Texture'].image = bpy.data.images['T']
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Emission Color'])
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Emission Color'].default_value = (0.2, 0.5, 1.0, 1.0)
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Emission Strength'].default_value = 2.0
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.3
bpy.ops.mesh.primitive_plane_add(location=(3, 0, 0))
bpy.context.object.data.color_attributes.new('Col', 'BYTE_COLOR', 'CORNER')
bpy.context.object.data.materials.append(bpy.data.materials.new('VC'))
bpy.data.materials['VC'].node_tree.nodes.new('ShaderNodeVertexColor')
bpy.data.materials['VC'].node_tree.nodes['Color Attribute'].layer_name = 'Col'
bpy.data.materials['VC'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.1, 0.2, 0.3, 1.0)
bpy.data.materials['VC'].node_tree.links.new(bpy.data.materials['VC'].node_tree.nodes['Color Attribute'].outputs['Color'], bpy.data.materials['VC'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.ops.export_scene.gltf(filepath='out/none.glb', export_image_format='NONE', export_all_vertex_colors=False)
bpy.ops.export_scene.gltf(filepath='out/auto.glb', export_all_vertex_colors=False)
