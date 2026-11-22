# WP 8.12: the exporter reads materials through `InlineShaderNodes`, which inlines node groups
# and passes through muted nodes. A node group holding an Image Texture (its Vector from the
# group input, unlinked: the group node's own value) feeds Base Color; a nested group passes a
# Value of 0.25 through Math (multiply by its unlinked group input 2.0) into Roughness (folded
# to 0.5); a muted Gamma on the emission texture's path is passed through; a muted Math with
# no linked input leaves Metallic at its own value. The glTF Material Output group has no
# Occlusion input, so it is not in the inlined tree and its Thickness is not read.
# golden: startup=factory tier=T2 compare=glb:out/groups.glb glb_structure
import bpy
bpy.data.images.new('Albedo', 4, 4)
bpy.data.images['Albedo'].generated_color = (0.9, 0.6, 0.3, 1.0)
bpy.data.images.new('Glow', 4, 4)
bpy.data.images['Glow'].generated_color = (0.2, 0.9, 0.4, 1.0)
bpy.data.node_groups.new('Tex', 'ShaderNodeTree')
bpy.data.node_groups['Tex'].interface.new_socket('Vector', in_out='INPUT', socket_type='NodeSocketVector')
bpy.data.node_groups['Tex'].interface.new_socket('Color', in_out='OUTPUT', socket_type='NodeSocketColor')
bpy.data.node_groups['Tex'].nodes.new('NodeGroupInput')
bpy.data.node_groups['Tex'].nodes.new('NodeGroupOutput')
bpy.data.node_groups['Tex'].nodes.new('ShaderNodeTexImage')
bpy.data.node_groups['Tex'].nodes['Image Texture'].image = bpy.data.images['Albedo']
bpy.data.node_groups['Tex'].links.new(bpy.data.node_groups['Tex'].nodes['Image Texture'].outputs['Color'], bpy.data.node_groups['Tex'].nodes['Group Output'].inputs['Color'])
bpy.data.node_groups.new('Scale', 'ShaderNodeTree')
bpy.data.node_groups['Scale'].interface.new_socket('Value', in_out='INPUT', socket_type='NodeSocketFloat')
bpy.data.node_groups['Scale'].interface.new_socket('By', in_out='INPUT', socket_type='NodeSocketFloat')
bpy.data.node_groups['Scale'].interface.new_socket('Result', in_out='OUTPUT', socket_type='NodeSocketFloat')
bpy.data.node_groups['Scale'].nodes.new('NodeGroupInput')
bpy.data.node_groups['Scale'].nodes.new('NodeGroupOutput')
bpy.data.node_groups['Scale'].nodes.new('ShaderNodeMath')
bpy.data.node_groups['Scale'].nodes['Math'].operation = 'MULTIPLY'
bpy.data.node_groups['Scale'].links.new(bpy.data.node_groups['Scale'].nodes['Group Input'].outputs['Value'], bpy.data.node_groups['Scale'].nodes['Math'].inputs[0])
bpy.data.node_groups['Scale'].links.new(bpy.data.node_groups['Scale'].nodes['Group Input'].outputs['By'], bpy.data.node_groups['Scale'].nodes['Math'].inputs[1])
bpy.data.node_groups['Scale'].links.new(bpy.data.node_groups['Scale'].nodes['Math'].outputs['Value'], bpy.data.node_groups['Scale'].nodes['Group Output'].inputs['Result'])
bpy.data.node_groups.new('Outer', 'ShaderNodeTree')
bpy.data.node_groups['Outer'].interface.new_socket('In', in_out='INPUT', socket_type='NodeSocketFloat')
bpy.data.node_groups['Outer'].interface.new_socket('Out', in_out='OUTPUT', socket_type='NodeSocketFloat')
bpy.data.node_groups['Outer'].nodes.new('NodeGroupInput')
bpy.data.node_groups['Outer'].nodes.new('NodeGroupOutput')
bpy.data.node_groups['Outer'].nodes.new('ShaderNodeGroup')
bpy.data.node_groups['Outer'].nodes['Group'].node_tree = bpy.data.node_groups['Scale']
bpy.data.node_groups['Outer'].nodes['Group'].inputs['By'].default_value = 2.0
bpy.data.node_groups['Outer'].links.new(bpy.data.node_groups['Outer'].nodes['Group Input'].outputs['In'], bpy.data.node_groups['Outer'].nodes['Group'].inputs['Value'])
bpy.data.node_groups['Outer'].links.new(bpy.data.node_groups['Outer'].nodes['Group'].outputs['Result'], bpy.data.node_groups['Outer'].nodes['Group Output'].inputs['Out'])
bpy.data.node_groups.new('glTF Material Output', 'ShaderNodeTree')
bpy.data.node_groups['glTF Material Output'].interface.new_socket('Thickness', socket_type='NodeSocketFloat')
bpy.data.node_groups['glTF Material Output'].nodes.new('NodeGroupInput')
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeGroup')
bpy.data.materials['Material'].node_tree.nodes['Group'].node_tree = bpy.data.node_groups['Tex']
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Group'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeValue')
bpy.data.materials['Material'].node_tree.nodes['Value'].outputs[0].default_value = 0.25
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeGroup')
bpy.data.materials['Material'].node_tree.nodes['Group.001'].node_tree = bpy.data.node_groups['Outer']
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Value'].outputs[0], bpy.data.materials['Material'].node_tree.nodes['Group.001'].inputs['In'])
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Group.001'].outputs['Out'], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Roughness'])
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Material'].node_tree.nodes['Image Texture'].image = bpy.data.images['Glow']
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeGamma')
bpy.data.materials['Material'].node_tree.nodes['Gamma'].mute = True
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Gamma'].inputs['Color'])
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Gamma'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Emission Color'])
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Emission Strength'].default_value = 1.0
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeMath')
bpy.data.materials['Material'].node_tree.nodes['Math'].mute = True
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Math'].outputs[0], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Metallic'])
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Metallic'].default_value = 0.3
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeGroup')
bpy.data.materials['Material'].node_tree.nodes['Group.002'].node_tree = bpy.data.node_groups['glTF Material Output']
bpy.data.materials['Material'].node_tree.nodes['Group.002'].inputs['Thickness'].default_value = 0.5
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Transmission Weight'].default_value = 1.0
bpy.ops.export_scene.gltf(filepath='out/groups.glb')
