# WP 3.5: ORM packing. Occlusion (an Image Texture through a Mix with white, factor 0.6, into the
# "glTF Material Output" group node's Occlusion), roughness and metallic (Image Textures plugged
# straight into the Principled sockets) from three images of the same size become one PNG
# named "Occ-Rough-Metal" (R occlusion, G roughness, B metallic) shared by occlusionTexture
# (strength 0.6) and metallicRoughnessTexture; the normal map (Strength 0.7) gives
# normalTexture.scale. A second material whose occlusion image has another size writes
# occlusion and metallic-roughness separately (one image holds metallic B and roughness G, so
# it is passed on unchanged as a happy-path re-encode).
# golden: startup=factory tier=T2 compare=glb:out/orm.glb glb_structure
import bpy
bpy.data.node_groups.new('glTF Material Output', 'ShaderNodeTree')
bpy.data.node_groups['glTF Material Output'].interface.new_socket('Occlusion', socket_type='NodeSocketFloat')
bpy.data.node_groups['glTF Material Output'].nodes.new('NodeGroupOutput')
bpy.data.node_groups['glTF Material Output'].nodes.new('NodeGroupInput')
bpy.data.images.new('Occ', 2, 2)
bpy.data.images['Occ'].generated_color = (0.25, 0.5, 0.75, 1.0)
bpy.data.images.new('Rough', 2, 2)
bpy.data.images['Rough'].generated_color = (0.1, 0.3, 0.9, 1.0)
bpy.data.images.new('Metal', 2, 2)
bpy.data.images['Metal'].generated_color = (0.6, 0.7, 0.2, 1.0)
bpy.data.images.new('Bumps', 2, 2)
bpy.data.images['Bumps'].generated_color = (0.5, 0.5, 1.0, 1.0)
bpy.data.images['Bumps'].colorspace_settings.name = 'Non-Color'
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Material'].node_tree.nodes['Image Texture'].image = bpy.data.images['Occ']
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Material'].node_tree.nodes['Image Texture.001'].image = bpy.data.images['Rough']
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Material'].node_tree.nodes['Image Texture.002'].image = bpy.data.images['Metal']
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Material'].node_tree.nodes['Image Texture.003'].image = bpy.data.images['Bumps']
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeGroup')
bpy.data.materials['Material'].node_tree.nodes['Group'].node_tree = bpy.data.node_groups['glTF Material Output']
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeMix')
bpy.data.materials['Material'].node_tree.nodes['Mix'].data_type = 'RGBA'
bpy.data.materials['Material'].node_tree.nodes['Mix'].inputs['Factor'].default_value = 0.6
bpy.data.materials['Material'].node_tree.nodes['Mix'].inputs['A'].default_value = (1.0, 1.0, 1.0, 1.0)
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Mix'].inputs['B'])
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Mix'].outputs['Result'], bpy.data.materials['Material'].node_tree.nodes['Group'].inputs['Occlusion'])
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Image Texture.001'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Roughness'])
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Image Texture.002'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Metallic'])
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeNormalMap')
bpy.data.materials['Material'].node_tree.nodes['Normal Map'].inputs['Strength'].default_value = 0.7
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Image Texture.003'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Normal Map'].inputs['Color'])
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Normal Map'].outputs['Normal'], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Normal'])
bpy.data.images.new('BigOcc', 4, 4)
bpy.data.images['BigOcc'].generated_color = (0.8, 0.8, 0.8, 1.0)
bpy.data.images.new('MR', 2, 2)
bpy.data.images['MR'].generated_color = (0.0, 0.4, 0.9, 1.0)
bpy.data.meshes['Cube'].materials.append(bpy.data.materials.new('Split'))
bpy.data.materials['Split'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Split'].node_tree.nodes['Image Texture'].image = bpy.data.images['BigOcc']
bpy.data.materials['Split'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Split'].node_tree.nodes['Image Texture.001'].image = bpy.data.images['MR']
bpy.data.materials['Split'].node_tree.nodes.new('ShaderNodeGroup')
bpy.data.materials['Split'].node_tree.nodes['Group'].node_tree = bpy.data.node_groups['glTF Material Output']
bpy.data.materials['Split'].node_tree.links.new(bpy.data.materials['Split'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Split'].node_tree.nodes['Group'].inputs['Occlusion'])
bpy.data.materials['Split'].node_tree.links.new(bpy.data.materials['Split'].node_tree.nodes['Image Texture.001'].outputs['Color'], bpy.data.materials['Split'].node_tree.nodes['Principled BSDF'].inputs['Roughness'])
bpy.data.materials['Split'].node_tree.links.new(bpy.data.materials['Split'].node_tree.nodes['Image Texture.001'].outputs['Color'], bpy.data.materials['Split'].node_tree.nodes['Principled BSDF'].inputs['Metallic'])
bpy.data.meshes['Cube'].polygons[0].material_index = 1
bpy.ops.export_scene.gltf(filepath='out/orm.glb')
