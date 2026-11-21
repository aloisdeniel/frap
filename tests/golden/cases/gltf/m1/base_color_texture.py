# WP 1.23: an Image Texture node on Base Color (a generated image written as PNG); UVs as
# TEXCOORD_0 with the V flip.
# golden: startup=factory tier=T2 compare=glb:out/textured.glb glb_structure
import bpy
bpy.data.images.new('Checker', 8, 8)
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Material'].node_tree.nodes['Image Texture'].image = bpy.data.images['Checker']
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.ops.export_scene.gltf(filepath='out/textured.glb')
