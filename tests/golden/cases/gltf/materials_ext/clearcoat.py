# WP 3.6: KHR_materials_clearcoat export. `Coat Weight`/`Coat Roughness` constants, a
# clearcoat normal map (Image Texture -> Normal Map, Strength 0.5 -> scale) on the first
# material; a second material whose coat roughness keeps Blender's 0.03 (not written, the add-on
# compares with BLENDER_COAT_ROUGHNESS) and a third with roughness 0 (written).
# golden: startup=factory tier=T2 compare=glb:out/clearcoat.glb
import bpy
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Coat Weight'].default_value = 0.8
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Coat Roughness'].default_value = 0.2
bpy.data.images.new('Bumps', 4, 4)
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Material'].node_tree.nodes['Image Texture'].image = bpy.data.images['Bumps']
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeNormalMap')
bpy.data.materials['Material'].node_tree.nodes['Normal Map'].inputs['Strength'].default_value = 0.5
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Normal Map'].inputs['Color'])
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Normal Map'].outputs['Normal'], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Coat Normal'])
bpy.data.materials.new('Lacquer')
bpy.data.materials['Lacquer'].node_tree.nodes['Principled BSDF'].inputs['Coat Weight'].default_value = 1.0
bpy.data.materials.new('Glaze')
bpy.data.materials['Glaze'].node_tree.nodes['Principled BSDF'].inputs['Coat Weight'].default_value = 0.5
bpy.data.materials['Glaze'].node_tree.nodes['Principled BSDF'].inputs['Coat Roughness'].default_value = 0.0
bpy.ops.mesh.primitive_plane_add(location=(3, 0, 0))
bpy.context.object.data.materials.append(bpy.data.materials['Lacquer'])
bpy.ops.mesh.primitive_plane_add(location=(-3, 0, 0))
bpy.context.object.data.materials.append(bpy.data.materials['Glaze'])
bpy.ops.export_scene.gltf(filepath='out/clearcoat.glb')
