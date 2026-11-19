# WP 3.9: an agent textures a crate. The cube gets a 2x2 base colour image with distinct
# pixels (passed on as a re-encoded PNG), a Non-Color normal map through a Normal Map node
# (Strength 0.8 -> normalTexture.scale) and a roughness factor; smart_project replaces the
# factory UVs. Round-trip law (§17.2): each tool imports its own export into an empty file and
# exports again; both re-exports compare canonically.
# golden: startup=factory tier=T2 compare=glb:out/crate.glb,glb:out/crate_rt.glb
import bpy
bpy.data.objects['Cube'].name = 'Crate'
bpy.data.materials['Material'].name = 'Wood'
bpy.data.images.new('Planks', 2, 2)
bpy.data.images['Planks'].pixels = [0.8, 0.5, 0.2, 1.0, 0.6, 0.35, 0.1, 1.0, 0.4, 0.25, 0.05, 1.0, 0.9, 0.7, 0.4, 1.0]
bpy.data.images.new('PlankNormals', 2, 2)
bpy.data.images['PlankNormals'].colorspace_settings.name = 'Non-Color'
bpy.data.images['PlankNormals'].pixels = [0.5, 0.5, 1.0, 1.0, 0.6, 0.5, 0.9, 1.0, 0.5, 0.4, 0.9, 1.0, 0.45, 0.55, 0.95, 1.0]
bpy.data.materials['Wood'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Wood'].node_tree.nodes['Image Texture'].image = bpy.data.images['Planks']
bpy.data.materials['Wood'].node_tree.links.new(bpy.data.materials['Wood'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Wood'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.data.materials['Wood'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Wood'].node_tree.nodes['Image Texture.001'].image = bpy.data.images['PlankNormals']
bpy.data.materials['Wood'].node_tree.nodes.new('ShaderNodeNormalMap')
bpy.data.materials['Wood'].node_tree.nodes['Normal Map'].inputs['Strength'].default_value = 0.8
bpy.data.materials['Wood'].node_tree.links.new(bpy.data.materials['Wood'].node_tree.nodes['Image Texture.001'].outputs['Color'], bpy.data.materials['Wood'].node_tree.nodes['Normal Map'].inputs['Color'])
bpy.data.materials['Wood'].node_tree.links.new(bpy.data.materials['Wood'].node_tree.nodes['Normal Map'].outputs['Normal'], bpy.data.materials['Wood'].node_tree.nodes['Principled BSDF'].inputs['Normal'])
bpy.data.materials['Wood'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.85
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.smart_project(island_margin=0.02)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.export_scene.gltf(filepath='out/crate.glb')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/crate.glb')
bpy.ops.export_scene.gltf(filepath='out/crate_rt.glb')
