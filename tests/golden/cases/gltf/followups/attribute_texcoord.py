# WP 7.7: the texCoord of an Image Texture fed by an Attribute node (type GEOMETRY): an
# attribute naming a UV map reads that map (UVMap.001 → texCoord 1), a FLOAT_VECTOR corner
# attribute is written as an extra TEXCOORD after the two UV maps and the texture reads it
# (texCoord 2, get_final_material's attr_indices); a third material reads the attribute through
# a Mapping node. The cube has UVMap (active render) and UVMap.001.
# golden: startup=factory tier=T2 compare=glb:out/attr.glb glb_structure
import bpy
bpy.data.meshes['Cube'].uv_layers.new(name='UVMap.001')
bpy.data.meshes['Cube'].attributes.new('Proj', 'FLOAT_VECTOR', 'CORNER')
bpy.data.meshes['Cube'].attributes['Proj'].data[0].vector = (0.25, 0.75, 9.0)
bpy.data.meshes['Cube'].attributes['Proj'].data[5].vector = (0.5, 0.125, 9.0)
bpy.data.images.new('A', 4, 4)
bpy.data.images['A'].generated_color = (0.9, 0.6, 0.3, 1.0)
bpy.data.images.new('B', 4, 4)
bpy.data.images['B'].generated_color = (0.1, 0.6, 0.9, 1.0)
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Material'].node_tree.nodes['Image Texture'].image = bpy.data.images['A']
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeAttribute')
bpy.data.materials['Material'].node_tree.nodes['Attribute'].attribute_name = 'UVMap.001'
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Attribute'].outputs['Vector'], bpy.data.materials['Material'].node_tree.nodes['Image Texture'].inputs['Vector'])
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.data.meshes['Cube'].materials.append(bpy.data.materials.new('Gen'))
bpy.data.materials['Gen'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Gen'].node_tree.nodes['Image Texture'].image = bpy.data.images['B']
bpy.data.materials['Gen'].node_tree.nodes.new('ShaderNodeAttribute')
bpy.data.materials['Gen'].node_tree.nodes['Attribute'].attribute_name = 'Proj'
bpy.data.materials['Gen'].node_tree.links.new(bpy.data.materials['Gen'].node_tree.nodes['Attribute'].outputs['Vector'], bpy.data.materials['Gen'].node_tree.nodes['Image Texture'].inputs['Vector'])
bpy.data.materials['Gen'].node_tree.links.new(bpy.data.materials['Gen'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Gen'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.data.meshes['Cube'].materials.append(bpy.data.materials.new('Mapped'))
bpy.data.materials['Mapped'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Mapped'].node_tree.nodes['Image Texture'].image = bpy.data.images['B']
bpy.data.materials['Mapped'].node_tree.nodes.new('ShaderNodeAttribute')
bpy.data.materials['Mapped'].node_tree.nodes['Attribute'].attribute_name = 'Proj'
bpy.data.materials['Mapped'].node_tree.nodes.new('ShaderNodeMapping')
bpy.data.materials['Mapped'].node_tree.nodes['Mapping'].inputs['Location'].default_value = (0.5, 0.0, 0.0)
bpy.data.materials['Mapped'].node_tree.links.new(bpy.data.materials['Mapped'].node_tree.nodes['Attribute'].outputs['Vector'], bpy.data.materials['Mapped'].node_tree.nodes['Mapping'].inputs['Vector'])
bpy.data.materials['Mapped'].node_tree.links.new(bpy.data.materials['Mapped'].node_tree.nodes['Mapping'].outputs['Vector'], bpy.data.materials['Mapped'].node_tree.nodes['Image Texture'].inputs['Vector'])
bpy.data.materials['Mapped'].node_tree.links.new(bpy.data.materials['Mapped'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Mapped'].node_tree.nodes['Principled BSDF'].inputs['Emission Color'])
bpy.data.materials['Mapped'].node_tree.nodes['Principled BSDF'].inputs['Emission Strength'].default_value = 1.0
bpy.data.meshes['Cube'].polygons[2].material_index = 1
bpy.data.meshes['Cube'].polygons[3].material_index = 1
bpy.data.meshes['Cube'].polygons[4].material_index = 2
bpy.ops.export_scene.gltf(filepath='out/attr.glb')
