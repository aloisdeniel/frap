# WP 3.1: the glTF add-on's `create_settings_group('glTF Material Output')` written out line by
# line (io_scene_gltf2/blender/com/material_helpers.py, 5.2.4), then `make_settings_node`: a
# group node in the material using it. The material comparator checks the group node's sockets
# (identifiers Socket_0..4, values from the items, 100 for the thickness minimum) and a link into
# Occlusion; group and interface values are copied onto objects.
# golden: startup=factory tier=T0 compare=materials,objects
import bpy
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
bpy.data.node_groups['glTF Material Output'].nodes['Group Input'].location = (-200, 0)
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeGroup')
bpy.data.materials['Material'].node_tree.nodes['Group'].node_tree = bpy.data.node_groups['glTF Material Output']
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeSeparateColor')
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Separate Color'].outputs['Red'], bpy.data.materials['Material'].node_tree.nodes['Group'].inputs['Occlusion'])
bpy.data.materials['Material'].node_tree.nodes['Group'].inputs['Thickness'].default_value = 0.25
bpy.data.materials['Material'].node_tree.nodes['Group'].inputs[4].default_value
bpy.data.objects['Cube'].location.x = bpy.data.node_groups['glTF Material Output'].users
bpy.data.objects['Cube'].location.y = bpy.data.node_groups['glTF Material Output'].nodes['Group Input'].outputs[4].default_value
bpy.data.objects['Cube'].location.z = bpy.data.node_groups['glTF Material Output'].nodes['Group Input'].location[0]
bpy.data.objects['Cube'].scale.x = bpy.data.node_groups['glTF Material Output'].interface.items_tree[4].default_value
bpy.data.objects['Cube'].scale.y = bpy.data.node_groups['glTF Material Output'].interface.items_tree['Occlusion'].min_value * 1e-38
bpy.data.objects['Cube'].scale.z = bpy.data.node_groups['glTF Material Output'].interface.items_tree['Occlusion'].index
bpy.data.objects['Light'].location.x = bpy.data.materials['Material'].node_tree.nodes['Group'].inputs['Occlusion'].link_limit
bpy.data.objects['Light'].location.y = bpy.data.node_groups['glTF Material Output'].nodes['Group Output'].inputs[0].link_limit
bpy.data.node_groups['glTF Material Output'].nodes['Group Input'].outputs['Dispersion'].default_value
bpy.data.node_groups['glTF Material Output'].nodes['Group Output'].inputs['Occlusion']
bpy.data.node_groups['glTF Material Output'].interface.items_tree['Occlusion'].bl_socket_idname
bpy.data.node_groups['glTF Material Output'].interface.active.name
bpy.data.objects['Camera'].name = bpy.data.node_groups['glTF Material Output'].interface.items_tree[4].identifier
bpy.data.objects['Light'].name = bpy.data.materials['Material'].node_tree.nodes['Group'].inputs[0].label
