# WP 7.7: KHR_animation_pointer channels of KHR_texture_transform (get_texture_transform_from_mapping_node's
# current_texture_transform, sampled by material_nodetree_caching). Cube: one POINT Mapping node (Location
# keyed, Rotation Z 0.3, Scale 2) feeds the base colour texture and the normal texture, whose Normal Map
# feeds Normal and Coat Normal with its Strength keyed: clearcoatNormalTexture/scale is registered first and
# normalTexture/scale is its additional pointer (a copy of the channel with its own accessors); the Mapping's
# offset goes to clearcoatNormalTexture first, then normalTexture and baseColorTexture as additional copies.
# Plane Tex: a TEXTURE Mapping at the identity with its Z rotation keyed: the static transform is empty and
# kept (KHR_texture_transform is animated), the offset is the inverted TRS, the rotation the raw socket.
# Plane Vec: a VECTOR Mapping with its Scale keyed exports no channel (the add-on writes the converted
# rotation into the scale). Plane Orm: one Image Texture through Separate Color into the glTF Material
# Output's Occlusion, Roughness and Metallic (the ORM image), its Mapping's Scale keyed: occlusionTexture
# first, metallicRoughnessTexture as additional.
# golden: startup=factory tier=T2 compare=glb:out/tt.glb glb_structure
import bpy
bpy.context.scene.frame_end = 5
bpy.data.images.new('C', 4, 4)
bpy.data.images['C'].generated_color = (0.9, 0.6, 0.3, 1.0)
bpy.data.images.new('N', 4, 4)
bpy.data.images['N'].generated_color = (0.5, 0.5, 1.0, 1.0)
bpy.data.images.new('T', 4, 4)
bpy.data.images['T'].generated_color = (0.2, 0.8, 0.4, 1.0)
bpy.data.images.new('V', 4, 4)
bpy.data.images['V'].generated_color = (0.7, 0.1, 0.5, 1.0)
bpy.data.images.new('O', 4, 4)
bpy.data.images['O'].generated_color = (1.0, 0.5, 0.25, 1.0)
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Material'].node_tree.nodes['Image Texture'].image = bpy.data.images['C']
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Material'].node_tree.nodes['Image Texture.001'].image = bpy.data.images['N']
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeMapping')
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeNormalMap')
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Mapping'].outputs['Vector'], bpy.data.materials['Material'].node_tree.nodes['Image Texture'].inputs['Vector'])
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Mapping'].outputs['Vector'], bpy.data.materials['Material'].node_tree.nodes['Image Texture.001'].inputs['Vector'])
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Image Texture.001'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Normal Map'].inputs['Color'])
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Normal Map'].outputs['Normal'], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Normal'])
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Normal Map'].outputs['Normal'], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Coat Normal'])
bpy.data.materials['Material'].node_tree.nodes['Mapping'].inputs['Rotation'].default_value = (0.0, 0.0, 0.3)
bpy.data.materials['Material'].node_tree.nodes['Mapping'].inputs['Scale'].default_value = (2.0, 2.0, 1.0)
bpy.data.materials['Material'].node_tree.nodes['Mapping'].inputs['Location'].keyframe_insert('default_value', frame=1)
bpy.data.materials['Material'].node_tree.nodes['Mapping'].inputs['Location'].default_value = (0.5, 0.25, 0.0)
bpy.data.materials['Material'].node_tree.nodes['Mapping'].inputs['Location'].keyframe_insert('default_value', frame=5)
bpy.data.materials['Material'].node_tree.nodes['Normal Map'].inputs['Strength'].default_value = 0.5
bpy.data.materials['Material'].node_tree.nodes['Normal Map'].inputs['Strength'].keyframe_insert('default_value', frame=1)
bpy.data.materials['Material'].node_tree.nodes['Normal Map'].inputs['Strength'].default_value = 2.0
bpy.data.materials['Material'].node_tree.nodes['Normal Map'].inputs['Strength'].keyframe_insert('default_value', frame=5)
bpy.ops.mesh.primitive_plane_add(location=(3, 0, 0))
bpy.context.object.data.materials.append(bpy.data.materials.new('Tex'))
bpy.data.materials['Tex'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Tex'].node_tree.nodes['Image Texture'].image = bpy.data.images['T']
bpy.data.materials['Tex'].node_tree.nodes.new('ShaderNodeMapping')
bpy.data.materials['Tex'].node_tree.nodes['Mapping'].vector_type = 'TEXTURE'
bpy.data.materials['Tex'].node_tree.links.new(bpy.data.materials['Tex'].node_tree.nodes['Mapping'].outputs['Vector'], bpy.data.materials['Tex'].node_tree.nodes['Image Texture'].inputs['Vector'])
bpy.data.materials['Tex'].node_tree.links.new(bpy.data.materials['Tex'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Tex'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.data.materials['Tex'].node_tree.nodes['Mapping'].inputs['Rotation'].keyframe_insert('default_value', frame=1)
bpy.data.materials['Tex'].node_tree.nodes['Mapping'].inputs['Rotation'].default_value = (0.0, 0.0, 0.5)
bpy.data.materials['Tex'].node_tree.nodes['Mapping'].inputs['Rotation'].keyframe_insert('default_value', frame=5)
bpy.data.materials['Tex'].node_tree.nodes['Mapping'].inputs['Rotation'].default_value = (0.0, 0.0, 0.0)
bpy.ops.mesh.primitive_plane_add(location=(-3, 0, 0))
bpy.context.object.data.materials.append(bpy.data.materials.new('Vec'))
bpy.data.materials['Vec'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Vec'].node_tree.nodes['Image Texture'].image = bpy.data.images['V']
bpy.data.materials['Vec'].node_tree.nodes.new('ShaderNodeMapping')
bpy.data.materials['Vec'].node_tree.nodes['Mapping'].vector_type = 'VECTOR'
bpy.data.materials['Vec'].node_tree.links.new(bpy.data.materials['Vec'].node_tree.nodes['Mapping'].outputs['Vector'], bpy.data.materials['Vec'].node_tree.nodes['Image Texture'].inputs['Vector'])
bpy.data.materials['Vec'].node_tree.links.new(bpy.data.materials['Vec'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Vec'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.data.materials['Vec'].node_tree.nodes['Mapping'].inputs['Rotation'].default_value = (0.0, 0.0, 0.3)
bpy.data.materials['Vec'].node_tree.nodes['Mapping'].inputs['Scale'].keyframe_insert('default_value', frame=1)
bpy.data.materials['Vec'].node_tree.nodes['Mapping'].inputs['Scale'].default_value = (2.0, 3.0, 1.0)
bpy.data.materials['Vec'].node_tree.nodes['Mapping'].inputs['Scale'].keyframe_insert('default_value', frame=5)
bpy.data.materials['Vec'].node_tree.nodes['Mapping'].inputs['Scale'].default_value = (1.0, 1.0, 1.0)
bpy.data.node_groups.new('glTF Material Output', 'ShaderNodeTree')
bpy.data.node_groups['glTF Material Output'].interface.new_socket('Occlusion', socket_type='NodeSocketFloat')
bpy.data.node_groups['glTF Material Output'].nodes.new('NodeGroupOutput')
bpy.data.node_groups['glTF Material Output'].nodes.new('NodeGroupInput')
bpy.ops.mesh.primitive_plane_add(location=(0, 3, 0))
bpy.context.object.data.materials.append(bpy.data.materials.new('Orm'))
bpy.data.materials['Orm'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Orm'].node_tree.nodes['Image Texture'].image = bpy.data.images['O']
bpy.data.materials['Orm'].node_tree.nodes.new('ShaderNodeMapping')
bpy.data.materials['Orm'].node_tree.nodes.new('ShaderNodeSeparateColor')
bpy.data.materials['Orm'].node_tree.nodes.new('ShaderNodeGroup')
bpy.data.materials['Orm'].node_tree.nodes['Group'].node_tree = bpy.data.node_groups['glTF Material Output']
bpy.data.materials['Orm'].node_tree.links.new(bpy.data.materials['Orm'].node_tree.nodes['Mapping'].outputs['Vector'], bpy.data.materials['Orm'].node_tree.nodes['Image Texture'].inputs['Vector'])
bpy.data.materials['Orm'].node_tree.links.new(bpy.data.materials['Orm'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Orm'].node_tree.nodes['Separate Color'].inputs['Color'])
bpy.data.materials['Orm'].node_tree.links.new(bpy.data.materials['Orm'].node_tree.nodes['Separate Color'].outputs['Red'], bpy.data.materials['Orm'].node_tree.nodes['Group'].inputs['Occlusion'])
bpy.data.materials['Orm'].node_tree.links.new(bpy.data.materials['Orm'].node_tree.nodes['Separate Color'].outputs['Green'], bpy.data.materials['Orm'].node_tree.nodes['Principled BSDF'].inputs['Roughness'])
bpy.data.materials['Orm'].node_tree.links.new(bpy.data.materials['Orm'].node_tree.nodes['Separate Color'].outputs['Blue'], bpy.data.materials['Orm'].node_tree.nodes['Principled BSDF'].inputs['Metallic'])
bpy.data.materials['Orm'].node_tree.nodes['Mapping'].inputs['Scale'].keyframe_insert('default_value', frame=1)
bpy.data.materials['Orm'].node_tree.nodes['Mapping'].inputs['Scale'].default_value = (2.0, 2.0, 1.0)
bpy.data.materials['Orm'].node_tree.nodes['Mapping'].inputs['Scale'].keyframe_insert('default_value', frame=5)
bpy.context.scene.frame_set(1)
bpy.ops.export_scene.gltf(filepath='out/tt.glb', export_animation_mode='SCENE', export_pointer_animation=True)
