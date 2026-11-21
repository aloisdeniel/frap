# WP 7.3: EXT_texture_webp export (io_scene_gltf2 5.2.41 texture.py `__gather_source` and
# `__gather_extensions`). Material `Material` samples a PNG file, `W` a lossless WebP file,
# `Alpha` the WebP file's colour and alpha (an Alpha socket keeps a WebP image WebP), `Packed`
# metallic and roughness from two images (a packed, unhappy-path image). Each export covers one
# combination of export_image_format, export_image_add_webp and export_image_webp_fallback;
# `webp` re-encodes the PNG lossy at the default quality 75, `webp100` lossless; `sep` is
# GLTF_SEPARATE (file names taken in the add-on's order: the WebP file's own image takes
# w.webp without being written, the extension's is w-1.webp). The GLBs are compared with
# Blender's by crates/frap-ops-io-tests/tests/gltf_textures.rs (structure, names, MIME types,
# URIs, decoded pixels; lossy WebP within a tolerance).
# golden: startup=factory tier=T2 compare=materials
# gltf-output: out/webp.glb out/webp100.glb out/add.glb out/fallback.glb out/add_fallback.glb out/jpeg_add.glb out/sep/sep.gltf
import bpy
bpy.data.images.new('A', 4, 2, alpha=True)
bpy.data.images['A'].pixels = [0.0, 0.1, 0.2, 1.0, 0.3, 0.4, 0.5, 1.0, 0.6, 0.7, 0.8, 1.0, 0.9, 1.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 0.5, 0.5, 0.5, 1.0]
bpy.data.images['A'].save(filepath='out/a.png')
bpy.data.images.new('B', 4, 2, alpha=True)
bpy.data.images['B'].pixels = [0.9, 0.8, 0.7, 1.0, 0.6, 0.5, 0.4, 0.5, 0.3, 0.2, 0.1, 1.0, 0.0, 0.25, 0.5, 0.75, 1.0, 1.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.25, 1.0, 0.0, 1.0, 1.0, 0.2, 0.4, 0.6, 1.0]
bpy.data.images['B'].file_format = 'WEBP'
bpy.data.images['B'].save(filepath='out/w.webp', quality=100)
bpy.data.images.load('out/a.png')
bpy.data.images.load('out/w.webp')
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Material'].node_tree.nodes['Image Texture'].image = bpy.data.images['a.png']
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.data.meshes['Cube'].materials.append(bpy.data.materials.new('W'))
bpy.data.materials['W'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['W'].node_tree.nodes['Image Texture'].image = bpy.data.images['w.webp']
bpy.data.materials['W'].node_tree.links.new(bpy.data.materials['W'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['W'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.data.meshes['Cube'].materials.append(bpy.data.materials.new('Alpha'))
bpy.data.materials['Alpha'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Alpha'].node_tree.nodes['Image Texture'].image = bpy.data.images['w.webp']
bpy.data.materials['Alpha'].node_tree.links.new(bpy.data.materials['Alpha'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Alpha'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.data.materials['Alpha'].node_tree.links.new(bpy.data.materials['Alpha'].node_tree.nodes['Image Texture'].outputs['Alpha'], bpy.data.materials['Alpha'].node_tree.nodes['Principled BSDF'].inputs['Alpha'])
bpy.data.meshes['Cube'].materials.append(bpy.data.materials.new('Packed'))
bpy.data.materials['Packed'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Packed'].node_tree.nodes['Image Texture'].image = bpy.data.images['a.png']
bpy.data.materials['Packed'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Packed'].node_tree.nodes['Image Texture.001'].image = bpy.data.images['w.webp']
bpy.data.materials['Packed'].node_tree.nodes.new('ShaderNodeSeparateColor')
bpy.data.materials['Packed'].node_tree.nodes.new('ShaderNodeSeparateColor')
bpy.data.materials['Packed'].node_tree.links.new(bpy.data.materials['Packed'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Packed'].node_tree.nodes['Separate Color'].inputs['Color'])
bpy.data.materials['Packed'].node_tree.links.new(bpy.data.materials['Packed'].node_tree.nodes['Image Texture.001'].outputs['Color'], bpy.data.materials['Packed'].node_tree.nodes['Separate Color.001'].inputs['Color'])
bpy.data.materials['Packed'].node_tree.links.new(bpy.data.materials['Packed'].node_tree.nodes['Separate Color'].outputs['Blue'], bpy.data.materials['Packed'].node_tree.nodes['Principled BSDF'].inputs['Metallic'])
bpy.data.materials['Packed'].node_tree.links.new(bpy.data.materials['Packed'].node_tree.nodes['Separate Color.001'].outputs['Green'], bpy.data.materials['Packed'].node_tree.nodes['Principled BSDF'].inputs['Roughness'])
bpy.data.meshes['Cube'].polygons[1].material_index = 1
bpy.data.meshes['Cube'].polygons[2].material_index = 2
bpy.data.meshes['Cube'].polygons[3].material_index = 3
bpy.ops.export_scene.gltf(filepath='out/webp.glb', export_image_format='WEBP')
bpy.ops.export_scene.gltf(filepath='out/webp100.glb', export_image_format='WEBP', export_image_quality=100)
bpy.ops.export_scene.gltf(filepath='out/add.glb', export_image_add_webp=True)
bpy.ops.export_scene.gltf(filepath='out/fallback.glb', export_image_webp_fallback=True)
bpy.ops.export_scene.gltf(filepath='out/add_fallback.glb', export_image_add_webp=True, export_image_webp_fallback=True)
bpy.ops.export_scene.gltf(filepath='out/jpeg_add.glb', export_image_format='JPEG', export_image_add_webp=True, export_image_webp_fallback=True, export_image_quality=90)
bpy.ops.export_scene.gltf(filepath='out/sep/sep.gltf', export_format='GLTF_SEPARATE', export_image_add_webp=True, export_image_webp_fallback=True)
