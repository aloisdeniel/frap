# WP 3.1: `node.inputs[key]` / `node.outputs[key]` match an available socket's identifier first,
# then its name (`find_socket_by_key`), except on Mix and Map Range (names only). An identifier
# wins over another socket's equal name. Values are copied onto objects (last).
# golden: startup=factory tier=T0 compare=materials,objects
import bpy
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeMath')
bpy.data.materials['Material'].node_tree.nodes['Math'].inputs['Value_001'].default_value = 0.25
bpy.data.objects['Cube'].location.x = bpy.data.materials['Material'].node_tree.nodes['Math'].inputs[1].default_value
bpy.data.materials['Material'].node_tree.nodes['Math'].inputs['Value_002']
bpy.data.materials['Material'].node_tree.nodes['Math'].inputs['Nope']
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].outputs['BSDF']
bpy.data.node_groups.new('G', 'ShaderNodeTree')
bpy.data.node_groups['G'].interface.new_socket('first')
bpy.data.node_groups['G'].interface.new_socket('Socket_0')
bpy.data.node_groups['G'].nodes.new('NodeGroupOutput')
bpy.data.node_groups['G'].nodes['Group Output'].inputs['__extend__']
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeGroup')
bpy.data.materials['Material'].node_tree.nodes['Group'].node_tree = bpy.data.node_groups['G']
bpy.data.materials['Material'].node_tree.nodes['Group'].inputs['Socket_1'].default_value = 3.0
bpy.data.materials['Material'].node_tree.nodes['Group'].inputs['Socket_0'].default_value = 4.0
bpy.data.objects['Cube'].location.y = bpy.data.materials['Material'].node_tree.nodes['Group'].inputs[0].default_value
bpy.data.objects['Cube'].location.z = bpy.data.materials['Material'].node_tree.nodes['Group'].inputs[1].default_value
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeMix')
bpy.data.materials['Material'].node_tree.nodes['Mix'].inputs['A_Float']
bpy.data.materials['Material'].node_tree.nodes['Mix'].inputs['A'].default_value = 0.125
bpy.data.objects['Light'].location.x = bpy.data.materials['Material'].node_tree.nodes['Mix'].inputs[2].default_value
bpy.data.materials['Material'].node_tree.nodes['Mix'].data_type = 'ROTATION'
bpy.data.materials['Material'].node_tree.nodes['Mix'].data_type = 'RGBA'
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeMapRange')
bpy.data.materials['Material'].node_tree.nodes['Map Range'].inputs['From_Min_FLOAT3']
bpy.data.materials['Material'].node_tree.nodes['Map Range'].inputs['From Min'].default_value = 0.5
bpy.data.objects['Light'].location.y = bpy.data.materials['Material'].node_tree.nodes['Map Range'].inputs[1].default_value
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeVectorMath')
bpy.data.materials['Material'].node_tree.nodes['Vector Math'].inputs['Vector_001'].default_value = (1, 2, 3)
bpy.data.objects['Light'].scale = bpy.data.materials['Material'].node_tree.nodes['Vector Math'].inputs[1].default_value
