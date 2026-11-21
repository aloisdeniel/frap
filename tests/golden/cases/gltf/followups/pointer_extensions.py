# WP 7.7: KHR_animation_pointer channels of the glTF Material Output group's sockets: an animated
# Iridescence Factor (and the Thin Film IOR, Thin Film Thickness the add-on registers with it),
# Dispersion (with a volume: Thickness 1, transmission 1) and the occlusion strength, the Factor
# of a Mix whose B is white between an Image Texture and the group's Occlusion: it exports
# reversed (1 - Factor, __gather_occlusion_strength's reverse); the Thin Film Thickness is 500.
# The Occlusion socket makes the add-on read the original tree anyway. Second file: a plane
# whose material animates only the Iridescence Factor with the default Thin Film Thickness 0:
# __check_iridescence drops the extension and its channels (no animation is left).
# golden: startup=factory tier=T2 compare=glb:out/ext.glb,glb:out/drop.glb glb_structure
import bpy
bpy.context.scene.frame_end = 5
bpy.data.node_groups.new('glTF Material Output', 'ShaderNodeTree')
bpy.data.node_groups['glTF Material Output'].interface.new_socket('Occlusion', socket_type='NodeSocketFloat')
bpy.data.node_groups['glTF Material Output'].interface.new_socket('Thickness', socket_type='NodeSocketFloat')
bpy.data.node_groups['glTF Material Output'].interface.items_tree['Thickness'].default_value = 0.0
bpy.data.node_groups['glTF Material Output'].interface.new_socket('Dispersion', socket_type='NodeSocketFloat')
bpy.data.node_groups['glTF Material Output'].interface.items_tree['Dispersion'].default_value = 0.0
bpy.data.node_groups['glTF Material Output'].interface.new_socket('Iridescence Factor', socket_type='NodeSocketFloat')
bpy.data.node_groups['glTF Material Output'].interface.items_tree['Iridescence Factor'].default_value = 0.0
bpy.data.node_groups['glTF Material Output'].interface.new_socket('Iridescence Thickness Minimum', socket_type='NodeSocketFloat')
bpy.data.node_groups['glTF Material Output'].interface.items_tree['Iridescence Thickness Minimum'].default_value = 100.0
bpy.data.node_groups['glTF Material Output'].nodes.new('NodeGroupOutput')
bpy.data.node_groups['glTF Material Output'].nodes.new('NodeGroupInput')
bpy.data.images.new('AO', 4, 4)
bpy.data.images['AO'].generated_color = (0.7, 0.7, 0.7, 1.0)
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Transmission Weight'].default_value = 1.0
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeGroup')
bpy.data.materials['Material'].node_tree.nodes['Group'].node_tree = bpy.data.node_groups['glTF Material Output']
bpy.data.materials['Material'].node_tree.nodes['Group'].inputs['Thickness'].default_value = 1.0
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Material'].node_tree.nodes['Image Texture'].image = bpy.data.images['AO']
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeMix')
bpy.data.materials['Material'].node_tree.nodes['Mix'].data_type = 'RGBA'
bpy.data.materials['Material'].node_tree.nodes['Mix'].inputs['B'].default_value = (1.0, 1.0, 1.0, 1.0)
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Mix'].inputs['A'])
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Mix'].outputs['Result'], bpy.data.materials['Material'].node_tree.nodes['Group'].inputs['Occlusion'])
bpy.data.materials['Material'].node_tree.nodes['Mix'].inputs['Factor'].default_value = 0.2
bpy.data.materials['Material'].node_tree.nodes['Mix'].inputs['Factor'].keyframe_insert('default_value', frame=1)
bpy.data.materials['Material'].node_tree.nodes['Mix'].inputs['Factor'].default_value = 0.6
bpy.data.materials['Material'].node_tree.nodes['Mix'].inputs['Factor'].keyframe_insert('default_value', frame=5)
bpy.data.materials['Material'].node_tree.nodes['Group'].inputs['Dispersion'].default_value = 0.1
bpy.data.materials['Material'].node_tree.nodes['Group'].inputs['Dispersion'].keyframe_insert('default_value', frame=1)
bpy.data.materials['Material'].node_tree.nodes['Group'].inputs['Dispersion'].default_value = 0.5
bpy.data.materials['Material'].node_tree.nodes['Group'].inputs['Dispersion'].keyframe_insert('default_value', frame=5)
bpy.data.materials['Material'].node_tree.nodes['Group'].inputs['Iridescence Factor'].default_value = 0.25
bpy.data.materials['Material'].node_tree.nodes['Group'].inputs['Iridescence Factor'].keyframe_insert('default_value', frame=1)
bpy.data.materials['Material'].node_tree.nodes['Group'].inputs['Iridescence Factor'].default_value = 1.0
bpy.data.materials['Material'].node_tree.nodes['Group'].inputs['Iridescence Factor'].keyframe_insert('default_value', frame=5)
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Thin Film IOR'].default_value = 1.5
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Thin Film IOR'].keyframe_insert('default_value', frame=1)
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Thin Film IOR'].default_value = 1.8
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Thin Film IOR'].keyframe_insert('default_value', frame=5)
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Thin Film Thickness'].default_value = 500.0
bpy.ops.export_scene.gltf(filepath='out/ext.glb', export_animation_mode='SCENE', export_pointer_animation=True)
bpy.ops.mesh.primitive_plane_add(location=(3, 0, 0))
bpy.context.object.data.materials.append(bpy.data.materials.new('Irid'))
bpy.data.materials['Irid'].node_tree.nodes.new('ShaderNodeGroup')
bpy.data.materials['Irid'].node_tree.nodes['Group'].node_tree = bpy.data.node_groups['glTF Material Output']
bpy.data.materials['Irid'].node_tree.nodes['Group'].inputs['Iridescence Factor'].default_value = 0.25
bpy.data.materials['Irid'].node_tree.nodes['Group'].inputs['Iridescence Factor'].keyframe_insert('default_value', frame=1)
bpy.data.materials['Irid'].node_tree.nodes['Group'].inputs['Iridescence Factor'].default_value = 1.0
bpy.data.materials['Irid'].node_tree.nodes['Group'].inputs['Iridescence Factor'].keyframe_insert('default_value', frame=5)
bpy.ops.export_scene.gltf(filepath='out/drop.glb', export_animation_mode='SCENE', export_pointer_animation=True, use_selection=True)
