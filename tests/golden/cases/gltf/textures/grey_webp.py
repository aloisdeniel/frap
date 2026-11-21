# WP 7.3: a greyscale (1-channel, 8-plane) PNG cannot be saved as WebP in Blender 5.2.4
# (OpenImageIO: "webp does not support 1-channel images"; `_encode_temp_image` logs "Error while
# saving image" and gets no bytes, then "Image data is empty, not exporting image"). Material
# `Material` samples grey.png (6x4 greyscale) as Base Color, `A` greya.png (4x4 grey + alpha,
# which WebP writes) as Base Color and Alpha. `webp` / `sep` (export_image_format='WEBP'): the
# grey texture keeps its sampler and has no image (`{"extensions": {}, "sampler": 0}`),
# EXT_texture_webp is used and required, no grey.webp is written; `add` / `add_fallback` /
# `sep_add` (export_image_add_webp): the grey texture keeps its PNG and gets no EXT_texture_webp,
# the grey-alpha one gets both. Files compared by crates/frap-ops-io-tests/tests/gltf_textures.rs.
# golden: startup=factory tier=T2 compare=materials
# gltf-output: out/webp.glb out/add.glb out/add_fallback.glb out/sep/sep.gltf out/sep_add/sep_add.gltf
import bpy
bpy.data.images.load('grey.png')
bpy.data.images.load('greya.png')
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Material'].node_tree.nodes['Image Texture'].image = bpy.data.images['grey.png']
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.data.meshes['Cube'].materials.append(bpy.data.materials.new('A'))
bpy.data.materials['A'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['A'].node_tree.nodes['Image Texture'].image = bpy.data.images['greya.png']
bpy.data.materials['A'].node_tree.links.new(bpy.data.materials['A'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['A'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.data.materials['A'].node_tree.links.new(bpy.data.materials['A'].node_tree.nodes['Image Texture'].outputs['Alpha'], bpy.data.materials['A'].node_tree.nodes['Principled BSDF'].inputs['Alpha'])
bpy.data.meshes['Cube'].polygons[1].material_index = 1
bpy.ops.export_scene.gltf(filepath='out/webp.glb', export_image_format='WEBP')
bpy.ops.export_scene.gltf(filepath='out/add.glb', export_image_add_webp=True)
bpy.ops.export_scene.gltf(filepath='out/add_fallback.glb', export_image_add_webp=True, export_image_webp_fallback=True)
bpy.ops.export_scene.gltf(filepath='out/sep/sep.gltf', export_format='GLTF_SEPARATE', export_image_format='WEBP')
bpy.ops.export_scene.gltf(filepath='out/sep_add/sep_add.gltf', export_format='GLTF_SEPARATE', export_image_add_webp=True)
