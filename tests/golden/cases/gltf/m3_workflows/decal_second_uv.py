# WP 3.9: a crate with a logo decal on a second UV map. The base colour texture reads the first
# map (texCoord 0); the decal image (distinct RGBA pixels) is mixed in as the emission colour
# through a UV Map node naming the second map 'DecalUV' (texCoord 1). Both maps export
# (TEXCOORD_0, TEXCOORD_1). Round trip (§17.2): the importer recreates the UV Map node and the
# re-export keeps texCoord 1.
# golden: startup=factory tier=T2 compare=glb:out/decal.glb,glb:out/decal_rt.glb
import bpy
bpy.data.meshes['Cube'].uv_layers.new(name='DecalUV')
bpy.data.meshes['Cube'].uv_layers['DecalUV'].data[0].uv = (0.1, 0.1)
bpy.data.meshes['Cube'].uv_layers['DecalUV'].data[1].uv = (0.9, 0.1)
bpy.data.meshes['Cube'].uv_layers['DecalUV'].data[2].uv = (0.9, 0.9)
bpy.data.meshes['Cube'].uv_layers['DecalUV'].data[3].uv = (0.1, 0.9)
bpy.data.images.new('Wood', 2, 2)
bpy.data.images['Wood'].pixels = [0.6, 0.4, 0.2, 1.0, 0.5, 0.3, 0.15, 1.0, 0.55, 0.35, 0.2, 1.0, 0.65, 0.45, 0.25, 1.0]
bpy.data.images.new('Logo', 2, 2, alpha=True)
bpy.data.images['Logo'].pixels = [1.0, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.5, 0.0, 0.0, 1.0, 0.25, 1.0, 1.0, 0.0, 0.0]
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Material'].node_tree.nodes['Image Texture'].image = bpy.data.images['Wood']
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeUVMap')
bpy.data.materials['Material'].node_tree.nodes['UV Map'].uv_map = 'DecalUV'
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Material'].node_tree.nodes['Image Texture.001'].image = bpy.data.images['Logo']
bpy.data.materials['Material'].node_tree.nodes['Image Texture.001'].extension = 'CLIP'
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['UV Map'].outputs['UV'], bpy.data.materials['Material'].node_tree.nodes['Image Texture.001'].inputs['Vector'])
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Image Texture.001'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Emission Color'])
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Emission Strength'].default_value = 1.0
bpy.ops.export_scene.gltf(filepath='out/decal.glb')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/decal.glb')
bpy.ops.export_scene.gltf(filepath='out/decal_rt.glb')
