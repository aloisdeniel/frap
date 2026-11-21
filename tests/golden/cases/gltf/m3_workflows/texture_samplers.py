# WP 3.9: sampler states from Image Texture settings on one image shared by four materials:
# Linear + REPEAT (the default sampler), Closest + EXTEND (NEAREST, CLAMP_TO_EDGE), Cubic +
# MIRROR (LINEAR filters, MIRRORED_REPEAT) and Smart + CLIP (LINEAR, CLAMP_TO_EDGE); four
# textures, four samplers, and the image is written once. Round trip (§17.2): the
# importer maps samplers back to interpolation and extension.
# golden: startup=factory tier=T2 compare=glb:out/samplers.glb,glb:out/samplers_rt.glb glb_structure
import bpy
bpy.data.images.new('Pattern', 2, 2)
bpy.data.images['Pattern'].pixels = [1.0, 0.5, 0.0, 1.0, 0.0, 0.5, 1.0, 1.0, 0.5, 1.0, 0.0, 1.0, 0.25, 0.25, 0.25, 1.0]
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Material'].node_tree.nodes['Image Texture'].image = bpy.data.images['Pattern']
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.data.meshes['Cube'].materials.append(bpy.data.materials.new('Pixel'))
bpy.data.materials['Pixel'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Pixel'].node_tree.nodes['Image Texture'].image = bpy.data.images['Pattern']
bpy.data.materials['Pixel'].node_tree.nodes['Image Texture'].interpolation = 'Closest'
bpy.data.materials['Pixel'].node_tree.nodes['Image Texture'].extension = 'EXTEND'
bpy.data.materials['Pixel'].node_tree.links.new(bpy.data.materials['Pixel'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Pixel'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.data.meshes['Cube'].materials.append(bpy.data.materials.new('Smooth'))
bpy.data.materials['Smooth'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Smooth'].node_tree.nodes['Image Texture'].image = bpy.data.images['Pattern']
bpy.data.materials['Smooth'].node_tree.nodes['Image Texture'].interpolation = 'Cubic'
bpy.data.materials['Smooth'].node_tree.nodes['Image Texture'].extension = 'MIRROR'
bpy.data.materials['Smooth'].node_tree.links.new(bpy.data.materials['Smooth'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Smooth'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.data.meshes['Cube'].materials.append(bpy.data.materials.new('Clipped'))
bpy.data.materials['Clipped'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Clipped'].node_tree.nodes['Image Texture'].image = bpy.data.images['Pattern']
bpy.data.materials['Clipped'].node_tree.nodes['Image Texture'].interpolation = 'Smart'
bpy.data.materials['Clipped'].node_tree.nodes['Image Texture'].extension = 'CLIP'
bpy.data.materials['Clipped'].node_tree.links.new(bpy.data.materials['Clipped'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Clipped'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.data.meshes['Cube'].polygons[1].material_index = 1
bpy.data.meshes['Cube'].polygons[2].material_index = 2
bpy.data.meshes['Cube'].polygons[3].material_index = 3
bpy.ops.export_scene.gltf(filepath='out/samplers.glb')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/samplers.glb')
bpy.ops.export_scene.gltf(filepath='out/samplers_rt.glb')
