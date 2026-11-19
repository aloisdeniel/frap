# WP 3.1: interface item identifiers come from a per-tree counter that only increases
# (`bNodeTreeInterface.next_uid`): after `clear()`, or after removing the last item, the next
# item does not reuse a freed `Socket_<n>`. Group nodes use the new identifier, so a value of an
# old socket never lands on the new one. Identifiers are copied onto object names (last).
# golden: startup=factory tier=T0 compare=materials,objects
import bpy
bpy.data.node_groups.new('G', 'ShaderNodeTree')
bpy.data.node_groups['G'].interface.new_socket('p')
bpy.data.node_groups['G'].interface.new_socket('q', socket_type='NodeSocketColor')
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeGroup')
bpy.data.materials['Material'].node_tree.nodes['Group'].node_tree = bpy.data.node_groups['G']
bpy.data.materials['Material'].node_tree.nodes['Group'].inputs['p'].default_value = 5.0
bpy.data.node_groups['G'].interface.clear()
bpy.data.node_groups['G'].interface.new_socket('z')
bpy.data.objects['Cube'].name = bpy.data.node_groups['G'].interface.items_tree['z'].identifier
bpy.data.objects['Light'].location.x = bpy.data.materials['Material'].node_tree.nodes['Group'].inputs['z'].default_value
bpy.data.node_groups['G'].interface.new_socket('last')
bpy.data.node_groups['G'].interface.remove(bpy.data.node_groups['G'].interface.items_tree['last'])
bpy.data.node_groups['G'].interface.new_socket('again', in_out='OUTPUT', socket_type='NodeSocketShader')
bpy.data.objects['Light'].name = bpy.data.node_groups['G'].interface.items_tree['again'].identifier
bpy.data.node_groups.new('H', 'ShaderNodeTree')
bpy.data.node_groups['H'].interface.new_socket('h')
bpy.data.objects['Camera'].name = bpy.data.node_groups['H'].interface.items_tree['h'].identifier
