# WP 3.1: `bpy.data.node_groups.remove(group)` clears `node_tree` on every group node using the
# group, and such a node loses its sockets and their links (as with `node_tree = None`): reading
# `inputs[0]` is an IndexError, a key a KeyError. Group nodes of other trees (a material, another
# group) are updated too. The group's link to the output replaced the default BSDF link.
# golden: startup=factory tier=T0 compare=materials
import bpy
bpy.data.node_groups.new('G', 'ShaderNodeTree')
bpy.data.node_groups['G'].interface.new_socket('k')
bpy.data.node_groups['G'].interface.new_socket('Out', in_out='OUTPUT', socket_type='NodeSocketShader')
bpy.data.materials.new('M2')
bpy.data.materials['M2'].node_tree.nodes.new('ShaderNodeGroup')
bpy.data.materials['M2'].node_tree.nodes['Group'].node_tree = bpy.data.node_groups['G']
bpy.data.materials['M2'].node_tree.nodes['Group'].inputs['k'].default_value = 2.0
bpy.data.materials['M2'].node_tree.links.new(bpy.data.materials['M2'].node_tree.nodes['Group'].outputs['Out'], bpy.data.materials['M2'].node_tree.nodes['Material Output'].inputs['Surface'])
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeGroup')
bpy.data.materials['Material'].node_tree.nodes['Group'].node_tree = bpy.data.node_groups['G']
bpy.data.node_groups.new('Outer', 'ShaderNodeTree')
bpy.data.node_groups['Outer'].nodes.new('ShaderNodeGroup')
bpy.data.node_groups['Outer'].nodes['Group'].node_tree = bpy.data.node_groups['G']
bpy.data.materials['M2'].node_tree.links[0].to_socket
bpy.data.node_groups.remove(bpy.data.node_groups['G'])
bpy.data.materials['M2'].node_tree.nodes['Group'].inputs[0]
bpy.data.materials['M2'].node_tree.nodes['Group'].inputs['k']
bpy.data.materials['M2'].node_tree.nodes['Group'].outputs[0]
bpy.data.materials['M2'].node_tree.links[0]
bpy.data.materials['Material'].node_tree.nodes['Group'].inputs[0]
bpy.data.node_groups['Outer'].nodes['Group'].inputs[0]
bpy.data.node_groups['Outer'].nodes['Group'].inputs['k']
bpy.data.materials['M2'].node_tree.nodes['Group'].node_tree
