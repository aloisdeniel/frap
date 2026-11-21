# WP 3.5: KHR_texture_transform from Mapping nodes, listed in extensionsUsed and
# extensionsRequired: a POINT mapping (location, Z rotation, scale) behind a UV Map node on the
# base colour (Closest, MIRROR → NEAREST and MIRRORED_REPEAT); a TEXTURE mapping (exported as
# its inverse) on the emission behind the importer's manual wrapping chain (Separate XYZ → Math
# WRAP 0..1 and PINGPONG 1 → Combine XYZ: wrapS REPEAT, wrapT MIRRORED_REPEAT); an identity
# mapping writes no transform.
# golden: startup=factory tier=T2 compare=glb:out/transform.glb glb_structure
import bpy
bpy.data.images.new('Tiles', 4, 4)
bpy.data.images['Tiles'].generated_color = (0.9, 0.6, 0.3, 1.0)
bpy.data.images.new('Glow', 2, 2)
bpy.data.images['Glow'].generated_color = (0.2, 0.9, 0.4, 1.0)
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Material'].node_tree.nodes['Image Texture'].image = bpy.data.images['Tiles']
bpy.data.materials['Material'].node_tree.nodes['Image Texture'].interpolation = 'Closest'
bpy.data.materials['Material'].node_tree.nodes['Image Texture'].extension = 'MIRROR'
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeMapping')
bpy.data.materials['Material'].node_tree.nodes['Mapping'].inputs['Location'].default_value = (0.1, 0.2, 0.0)
bpy.data.materials['Material'].node_tree.nodes['Mapping'].inputs['Rotation'].default_value = (0.0, 0.0, 0.3)
bpy.data.materials['Material'].node_tree.nodes['Mapping'].inputs['Scale'].default_value = (2.0, 3.0, 1.0)
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeUVMap')
bpy.data.materials['Material'].node_tree.nodes['UV Map'].uv_map = 'UVMap'
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['UV Map'].outputs['UV'], bpy.data.materials['Material'].node_tree.nodes['Mapping'].inputs['Vector'])
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Mapping'].outputs['Vector'], bpy.data.materials['Material'].node_tree.nodes['Image Texture'].inputs['Vector'])
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Material'].node_tree.nodes['Image Texture.001'].image = bpy.data.images['Glow']
bpy.data.materials['Material'].node_tree.nodes['Image Texture.001'].extension = 'EXTEND'
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeMapping')
bpy.data.materials['Material'].node_tree.nodes['Mapping.001'].vector_type = 'TEXTURE'
bpy.data.materials['Material'].node_tree.nodes['Mapping.001'].inputs['Location'].default_value = (0.5, 0.25, 0.0)
bpy.data.materials['Material'].node_tree.nodes['Mapping.001'].inputs['Scale'].default_value = (2.0, 2.0, 1.0)
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeSeparateXYZ')
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeCombineXYZ')
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeMath')
bpy.data.materials['Material'].node_tree.nodes['Math'].operation = 'WRAP'
bpy.data.materials['Material'].node_tree.nodes['Math'].inputs[1].default_value = 0.0
bpy.data.materials['Material'].node_tree.nodes['Math'].inputs[2].default_value = 1.0
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeMath')
bpy.data.materials['Material'].node_tree.nodes['Math.001'].operation = 'PINGPONG'
bpy.data.materials['Material'].node_tree.nodes['Math.001'].inputs[1].default_value = 1.0
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Mapping.001'].outputs['Vector'], bpy.data.materials['Material'].node_tree.nodes['Separate XYZ'].inputs['Vector'])
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Separate XYZ'].outputs['X'], bpy.data.materials['Material'].node_tree.nodes['Math'].inputs[0])
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Separate XYZ'].outputs['Y'], bpy.data.materials['Material'].node_tree.nodes['Math.001'].inputs[0])
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Math'].outputs[0], bpy.data.materials['Material'].node_tree.nodes['Combine XYZ'].inputs['X'])
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Math.001'].outputs[0], bpy.data.materials['Material'].node_tree.nodes['Combine XYZ'].inputs['Y'])
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Combine XYZ'].outputs['Vector'], bpy.data.materials['Material'].node_tree.nodes['Image Texture.001'].inputs['Vector'])
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Image Texture.001'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Emission Color'])
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Emission Strength'].default_value = 1.0
bpy.data.meshes['Cube'].materials.append(bpy.data.materials.new('Plain'))
bpy.data.materials['Plain'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Plain'].node_tree.nodes['Image Texture'].image = bpy.data.images['Tiles']
bpy.data.materials['Plain'].node_tree.nodes.new('ShaderNodeMapping')
bpy.data.materials['Plain'].node_tree.links.new(bpy.data.materials['Plain'].node_tree.nodes['Mapping'].outputs['Vector'], bpy.data.materials['Plain'].node_tree.nodes['Image Texture'].inputs['Vector'])
bpy.data.materials['Plain'].node_tree.links.new(bpy.data.materials['Plain'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Plain'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.data.meshes['Cube'].polygons[0].material_index = 1
bpy.ops.export_scene.gltf(filepath='out/transform.glb')
