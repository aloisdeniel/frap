# WP 3.1: editing a group interface updates the group's input/output nodes and every group node
# using it: outputs are kept before inputs, 'DEFAULT' is a float, item renames rename the
# sockets, removing an item removes its sockets and links, existing socket values survive, a
# later item default does not change existing sockets, `clear()` empties the group node.
# Assigning the material's own tree or a geometry group to `node_tree` is ignored, `None`
# removes the sockets.
# golden: startup=factory tier=T0 compare=materials,objects
import bpy
bpy.data.node_groups.new('G', 'ShaderNodeTree')
bpy.data.node_groups['G'].interface.new_socket('A')
bpy.data.node_groups['G'].interface.new_socket('Col', in_out='INPUT', socket_type='NodeSocketColor')
bpy.data.node_groups['G'].interface.new_socket('Out', in_out='OUTPUT', socket_type='NodeSocketShader')
bpy.data.node_groups['G'].interface.new_socket('V', socket_type='NodeSocketVector')
bpy.data.node_groups['G'].interface.new_socket('X', socket_type='NodeSocketGeometry')
bpy.data.node_groups['G'].nodes.new('NodeGroupInput')
bpy.data.node_groups['G'].nodes.new('NodeGroupOutput')
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeGroup')
bpy.data.materials['Material'].node_tree.nodes['Group'].node_tree = bpy.data.node_groups['G']
bpy.data.materials['Material'].node_tree.nodes['Group'].inputs['A'].default_value = 3.0
bpy.data.materials['Material'].node_tree.nodes['Group'].inputs['Col'].default_value = (0.5, 0.25, 1, 1)
bpy.data.node_groups['G'].interface.items_tree['A'].default_value = 9.0
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Group'].outputs['Out'], bpy.data.materials['Material'].node_tree.nodes['Material Output'].inputs['Surface'])
bpy.data.node_groups['G'].interface.new_socket('Late', socket_type='NodeSocketFloat')
bpy.data.node_groups['G'].interface.items_tree['Late'].name = 'Renamed'
bpy.data.materials['Material'].node_tree.nodes['Group'].inputs['Renamed'].default_value = 0.75
bpy.data.objects['Cube'].location = bpy.data.materials['Material'].node_tree.nodes['Group'].inputs['Col'].default_value[0:3]
bpy.data.objects['Cube'].scale.x = bpy.data.materials['Material'].node_tree.nodes['Group'].inputs['A'].default_value
bpy.data.objects['Cube'].scale.y = bpy.data.node_groups['G'].nodes['Group Input'].outputs['A'].default_value
bpy.data.objects['Cube'].scale.z = bpy.data.node_groups['G'].interface.items_tree['Out'].position
bpy.data.objects['Light'].location.x = bpy.data.node_groups['G'].interface.items_tree['A'].index
bpy.data.objects['Light'].location.y = bpy.data.materials['Material'].node_tree.nodes['Group'].inputs['Col'].default_value[3]
bpy.data.node_groups['G'].nodes['Group Input'].outputs['Renamed']
bpy.data.node_groups['G'].nodes['Group Output'].inputs['Out']
bpy.data.materials['Material'].node_tree.nodes['Group'].node_tree = bpy.data.materials['Material'].node_tree
bpy.data.node_groups.new('Geo', 'GeometryNodeTree')
bpy.data.materials['Material'].node_tree.nodes['Group'].node_tree = bpy.data.node_groups['Geo']
bpy.data.materials['Material'].node_tree.nodes['Group'].node_tree.name
bpy.data.node_groups['G'].interface.remove(bpy.data.node_groups['G'].interface.items_tree['Out'])
bpy.data.materials['Material'].node_tree.nodes['Group'].outputs['Out']
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeGroup')
bpy.data.materials['Material'].node_tree.nodes['Group.001'].node_tree = bpy.data.node_groups['G']
bpy.data.objects['Light'].location.z = bpy.data.node_groups['G'].users
bpy.data.materials['Material'].node_tree.nodes['Group.001'].node_tree = None
bpy.data.materials['Material'].node_tree.nodes['Group.001'].inputs['A']
bpy.data.objects['Light'].scale.x = bpy.data.node_groups['G'].users
bpy.data.node_groups['G'].interface.clear()
bpy.data.materials['Material'].node_tree.nodes['Group'].inputs['A']
bpy.data.objects['Camera'].name = bpy.data.node_groups['G'].interface.items_tree[0].name
