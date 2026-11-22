# WP 8.12: an alpha from another image is packed into the base colour image's alpha
# (`__get_image_data_mapping` with the Base Color and Alpha sockets: the colour image whole,
# then A from the alpha socket). Three materials: Alpha fed by a second image's Alpha output;
# Alpha fed by a second image's Color output (RGB → BW: 0.2989 R + 0.5870 G + 0.1140 B, times
# the image's alpha); and that grey alpha alone (a white image carrying it). The images are
# painted with per-pixel values so the packing is visible in the pixels.
# golden: startup=factory tier=T2 compare=glb:out/alpha.glb glb_structure
import bpy
bpy.data.images.new('Col', 2, 2, alpha=True)
bpy.data.images['Col'].pixels = [0.9, 0.1, 0.1, 1.0, 0.1, 0.9, 0.1, 1.0, 0.1, 0.1, 0.9, 1.0, 0.5, 0.5, 0.5, 1.0]
bpy.data.images.new('Mask', 2, 2, alpha=True)
bpy.data.images['Mask'].pixels = [0.2, 0.4, 0.6, 0.25, 0.8, 0.6, 0.4, 0.5, 1.0, 1.0, 1.0, 0.75, 0.0, 0.0, 0.0, 1.0]
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Material'].node_tree.nodes['Image Texture'].image = bpy.data.images['Col']
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Material'].node_tree.nodes['Image Texture.001'].image = bpy.data.images['Mask']
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Image Texture.001'].outputs['Alpha'], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Alpha'])
bpy.data.materials.new('Grey')
bpy.data.materials['Grey'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Grey'].node_tree.nodes['Image Texture'].image = bpy.data.images['Col']
bpy.data.materials['Grey'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Grey'].node_tree.nodes['Image Texture.001'].image = bpy.data.images['Mask']
bpy.data.materials['Grey'].node_tree.links.new(bpy.data.materials['Grey'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Grey'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.data.materials['Grey'].node_tree.links.new(bpy.data.materials['Grey'].node_tree.nodes['Image Texture.001'].outputs['Color'], bpy.data.materials['Grey'].node_tree.nodes['Principled BSDF'].inputs['Alpha'])
bpy.data.materials.new('GreyAlone')
bpy.data.materials['GreyAlone'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['GreyAlone'].node_tree.nodes['Image Texture'].image = bpy.data.images['Mask']
bpy.data.materials['GreyAlone'].node_tree.links.new(bpy.data.materials['GreyAlone'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['GreyAlone'].node_tree.nodes['Principled BSDF'].inputs['Alpha'])
bpy.ops.mesh.primitive_plane_add(location=(3, 0, 0))
bpy.context.object.data.materials.append(bpy.data.materials['Grey'])
bpy.ops.mesh.primitive_plane_add(location=(-3, 0, 0))
bpy.context.object.data.materials.append(bpy.data.materials['GreyAlone'])
bpy.ops.export_scene.gltf(filepath='out/alpha.glb')
