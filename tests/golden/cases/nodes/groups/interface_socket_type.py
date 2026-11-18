# WP 3.1: `socket_type` of an interface item and of `new_socket` depends on the tree type.
# Writing an item's `socket_type` changes the item's type (and struct), resets its value, and
# the sockets it declares (Group Input, group nodes) change type, take the item's value and keep
# their links; types of another tree type (and 'DEFAULT') are enum TypeErrors listing the tree's
# items. A geometry tree takes geometry sockets. Values are copied onto objects, names onto
# material names.
# golden: startup=factory tier=T0 compare=materials,objects
import bpy
bpy.data.materials.new('N1')
bpy.data.materials.new('N2')
bpy.data.materials.new('N3')
bpy.data.node_groups.new('G', 'ShaderNodeTree')
bpy.data.node_groups['G'].interface.new_socket('a')
bpy.data.node_groups['G'].nodes.new('NodeGroupInput')
bpy.data.node_groups['G'].nodes.new('ShaderNodeMath')
bpy.data.node_groups['G'].links.new(bpy.data.node_groups['G'].nodes['Group Input'].outputs['a'], bpy.data.node_groups['G'].nodes['Math'].inputs[0])
bpy.data.node_groups['G'].interface.items_tree['a'].default_value = 0.7
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeGroup')
bpy.data.materials['Material'].node_tree.nodes['Group'].node_tree = bpy.data.node_groups['G']
bpy.data.materials['Material'].node_tree.nodes['Group'].inputs['a'].default_value = 0.3
bpy.data.node_groups['G'].interface.items_tree['a'].socket_type = 'NodeSocketColor'
bpy.data.materials['N1'].name = bpy.data.node_groups['G'].interface.items_tree['a'].bl_socket_idname
bpy.data.objects['Cube'].location = bpy.data.materials['Material'].node_tree.nodes['Group'].inputs['a'].default_value[1:4]
bpy.data.objects['Cube'].scale = bpy.data.node_groups['G'].interface.items_tree['a'].default_value[1:4]
bpy.data.objects['Light'].location.x = bpy.data.node_groups['G'].links[0].from_socket.default_value[3]
bpy.data.objects['Light'].location.y = bpy.data.node_groups['G'].nodes['Group Input'].outputs['a'].default_value[3]
bpy.data.node_groups['G'].interface.items_tree['a'].socket_type = 'NodeSocketGeometry'
bpy.data.node_groups['G'].interface.items_tree['a'].socket_type = 'DEFAULT'
bpy.data.node_groups['G'].interface.items_tree['a'].socket_type = 'NodeSocketInt'
bpy.data.node_groups['G'].interface.items_tree['a'].default_value = 4
bpy.data.node_groups['G'].interface.items_tree['a'].socket_type = 'NodeSocketInt'
bpy.data.objects['Light'].location.z = bpy.data.node_groups['G'].interface.items_tree['a'].default_value
bpy.data.materials['N2'].name = bpy.data.materials['Material'].node_tree.nodes['Group'].inputs['a'].bl_idname
bpy.data.node_groups['G'].interface.new_socket('g', socket_type='NodeSocketGeometry')
bpy.data.node_groups['G'].interface.new_socket('d', socket_type='DEFAULT')
bpy.data.node_groups.new('Geo', 'GeometryNodeTree')
bpy.data.node_groups['Geo'].interface.new_socket('g', socket_type='NodeSocketGeometry')
bpy.data.node_groups['Geo'].interface.new_socket('o', in_out='OUTPUT', socket_type='NodeSocketGeometry')
bpy.data.node_groups['Geo'].interface.new_socket('s', socket_type='NodeSocketShader')
bpy.data.node_groups['Geo'].nodes.new('NodeGroupInput')
bpy.data.materials['N3'].name = bpy.data.node_groups['Geo'].nodes['Group Input'].outputs['g'].bl_idname
bpy.data.objects['Camera'].location.x = bpy.data.node_groups['Geo'].interface.items_tree['g'].position
bpy.data.node_groups['Geo'].interface.items_tree['g'].socket_type = 'NodeSocketFloat'
bpy.data.objects['Camera'].location.y = bpy.data.node_groups['Geo'].nodes['Group Input'].outputs['g'].default_value
