# WP 3.5: image passthrough vs re-encode. A PNG file loaded with images.load is copied as it
# is (named after the file); the same file loaded again and then edited (pixels, so is_dirty)
# is re-encoded with the edits; a generated UV_GRID and a scaled image are encoded from their
# pixels (Image::acquire); a texture whose image file is missing (size (0, 0)) is not written.
# golden: startup=factory tier=T2 compare=glb:out/sources.glb
import bpy
bpy.data.images.new('A', 4, 2, alpha=True)
bpy.data.images['A'].pixels = [0.0, 0.1, 0.2, 1.0, 0.3, 0.4, 0.5, 0.5, 0.6, 0.7, 0.8, 1.0, 0.9, 1.0, 0.0, 0.25, 1.0, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 0.5, 0.5, 0.5, 0.0]
bpy.data.images['A'].save(filepath='out/a.png')
bpy.data.images.load('out/a.png')
bpy.data.images.load('out/a.png')
bpy.data.images['a.png.001'].pixels[0:4] = (1.0, 1.0, 0.0, 1.0)
bpy.ops.image.new(name='Grid', width=16, height=8, generated_type='UV_GRID')
bpy.data.images.new('S', 3, 2)
bpy.data.images['S'].pixels[0:8] = (1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 1.0, 1.0)
bpy.data.images['S'].scale(6, 4)
bpy.data.images.load('out/a.png')
bpy.data.images['a.png.002'].filepath = 'out/nothere.png'
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Material'].node_tree.nodes['Image Texture'].image = bpy.data.images['a.png']
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.data.meshes['Cube'].materials.append(bpy.data.materials.new('Edited'))
bpy.data.materials['Edited'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Edited'].node_tree.nodes['Image Texture'].image = bpy.data.images['a.png.001']
bpy.data.materials['Edited'].node_tree.links.new(bpy.data.materials['Edited'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Edited'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.data.meshes['Cube'].materials.append(bpy.data.materials.new('Grid'))
bpy.data.materials['Grid'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Grid'].node_tree.nodes['Image Texture'].image = bpy.data.images['Grid']
bpy.data.materials['Grid'].node_tree.links.new(bpy.data.materials['Grid'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Grid'].node_tree.nodes['Principled BSDF'].inputs['Emission Color'])
bpy.data.materials['Grid'].node_tree.nodes['Principled BSDF'].inputs['Emission Strength'].default_value = 1.0
bpy.data.meshes['Cube'].materials.append(bpy.data.materials.new('Scaled'))
bpy.data.materials['Scaled'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Scaled'].node_tree.nodes['Image Texture'].image = bpy.data.images['S']
bpy.data.materials['Scaled'].node_tree.links.new(bpy.data.materials['Scaled'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Scaled'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.data.meshes['Cube'].materials.append(bpy.data.materials.new('Missing'))
bpy.data.materials['Missing'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Missing'].node_tree.nodes['Image Texture'].image = bpy.data.images['a.png.002']
bpy.data.materials['Missing'].node_tree.links.new(bpy.data.materials['Missing'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Missing'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.data.meshes['Cube'].polygons[4].material_index = 4
bpy.data.meshes['Cube'].polygons[1].material_index = 1
bpy.data.meshes['Cube'].polygons[2].material_index = 2
bpy.data.meshes['Cube'].polygons[3].material_index = 3
bpy.ops.export_scene.gltf(filepath='out/sources.glb')
