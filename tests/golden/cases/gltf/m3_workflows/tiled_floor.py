# WP 3.9: a tiled floor. One Mapping (POINT: location, Z rotation, scale 4x4) behind a UV Map
# node feeds both the tile colour (Closest, REPEAT) and its normal map (Linear, MIRROR): both
# textureInfos carry KHR_texture_transform (in extensionsUsed and extensionsRequired). The
# skirting board's texture uses EXTEND (CLAMP_TO_EDGE) without a mapping. Round trip (§17.2):
# the importer rebuilds the Mapping nodes and the re-export writes the same transforms.
# golden: startup=factory tier=T2 compare=glb:out/floor.glb,glb:out/floor_rt.glb glb_structure
import bpy
bpy.data.objects.remove(bpy.data.objects['Cube'])
bpy.ops.mesh.primitive_plane_add(size=8)
bpy.data.objects['Plane'].name = 'Floor'
bpy.data.materials['Material'].name = 'Tiles'
bpy.data.objects['Floor'].data.materials.append(bpy.data.materials['Tiles'])
bpy.data.images.new('TileColor', 2, 2)
bpy.data.images['TileColor'].pixels = [0.9, 0.9, 0.85, 1.0, 0.2, 0.2, 0.25, 1.0, 0.2, 0.2, 0.25, 1.0, 0.9, 0.9, 0.85, 1.0]
bpy.data.images.new('TileNormal', 2, 2)
bpy.data.images['TileNormal'].colorspace_settings.name = 'Non-Color'
bpy.data.images['TileNormal'].pixels = [0.5, 0.5, 1.0, 1.0, 0.55, 0.5, 0.95, 1.0, 0.5, 0.55, 0.95, 1.0, 0.45, 0.45, 0.9, 1.0]
bpy.data.materials['Tiles'].node_tree.nodes.new('ShaderNodeUVMap')
bpy.data.materials['Tiles'].node_tree.nodes['UV Map'].uv_map = 'UVMap'
bpy.data.materials['Tiles'].node_tree.nodes.new('ShaderNodeMapping')
bpy.data.materials['Tiles'].node_tree.nodes['Mapping'].inputs['Location'].default_value = (0.25, 0.5, 0.0)
bpy.data.materials['Tiles'].node_tree.nodes['Mapping'].inputs['Rotation'].default_value = (0.0, 0.0, 0.5)
bpy.data.materials['Tiles'].node_tree.nodes['Mapping'].inputs['Scale'].default_value = (4.0, 4.0, 1.0)
bpy.data.materials['Tiles'].node_tree.links.new(bpy.data.materials['Tiles'].node_tree.nodes['UV Map'].outputs['UV'], bpy.data.materials['Tiles'].node_tree.nodes['Mapping'].inputs['Vector'])
bpy.data.materials['Tiles'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Tiles'].node_tree.nodes['Image Texture'].image = bpy.data.images['TileColor']
bpy.data.materials['Tiles'].node_tree.nodes['Image Texture'].interpolation = 'Closest'
bpy.data.materials['Tiles'].node_tree.links.new(bpy.data.materials['Tiles'].node_tree.nodes['Mapping'].outputs['Vector'], bpy.data.materials['Tiles'].node_tree.nodes['Image Texture'].inputs['Vector'])
bpy.data.materials['Tiles'].node_tree.links.new(bpy.data.materials['Tiles'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Tiles'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.data.materials['Tiles'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Tiles'].node_tree.nodes['Image Texture.001'].image = bpy.data.images['TileNormal']
bpy.data.materials['Tiles'].node_tree.nodes['Image Texture.001'].extension = 'MIRROR'
bpy.data.materials['Tiles'].node_tree.links.new(bpy.data.materials['Tiles'].node_tree.nodes['Mapping'].outputs['Vector'], bpy.data.materials['Tiles'].node_tree.nodes['Image Texture.001'].inputs['Vector'])
bpy.data.materials['Tiles'].node_tree.nodes.new('ShaderNodeNormalMap')
bpy.data.materials['Tiles'].node_tree.links.new(bpy.data.materials['Tiles'].node_tree.nodes['Image Texture.001'].outputs['Color'], bpy.data.materials['Tiles'].node_tree.nodes['Normal Map'].inputs['Color'])
bpy.data.materials['Tiles'].node_tree.links.new(bpy.data.materials['Tiles'].node_tree.nodes['Normal Map'].outputs['Normal'], bpy.data.materials['Tiles'].node_tree.nodes['Principled BSDF'].inputs['Normal'])
bpy.ops.mesh.primitive_cube_add(size=1, location=(0, 4, 0.25), scale=(8, 0.1, 0.5))
bpy.data.objects['Cube'].name = 'Skirting'
bpy.data.materials.new('Skirt')
bpy.data.objects['Skirting'].data.materials.append(bpy.data.materials['Skirt'])
bpy.data.materials['Skirt'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Skirt'].node_tree.nodes['Image Texture'].image = bpy.data.images['TileColor']
bpy.data.materials['Skirt'].node_tree.nodes['Image Texture'].extension = 'EXTEND'
bpy.data.materials['Skirt'].node_tree.links.new(bpy.data.materials['Skirt'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Skirt'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.ops.export_scene.gltf(filepath='out/floor.glb')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/floor.glb')
bpy.ops.export_scene.gltf(filepath='out/floor_rt.glb')
