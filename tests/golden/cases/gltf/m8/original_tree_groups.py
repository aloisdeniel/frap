# WP 8.12: the add-on reads the original node tree (`__can_use_inline` false) when a glTF
# Material Output group has an Occlusion input, or with `export_unused_textures`; its
# navigation (`NodeNav.move_back`, `from_socket`) still goes into node groups through their
# Group Output and back out through their Group Input. Base Color comes from an Image Texture
# through a pass-through group, Roughness and Metallic from a Separate Color inside a group;
# a group whose Group Output input is unlinked feeds Emission Color (a dead end: no constant,
# no texture). `occ.glb` has the Occlusion group, `unused.glb` the same tree without it under
# `export_unused_textures` (the textures read through groups are used, so not appended again).
# golden: startup=factory tier=T2 compare=glb:out/occ.glb,glb:out/unused.glb glb_structure
import bpy
bpy.data.images.new('Albedo', 4, 4)
bpy.data.images['Albedo'].generated_color = (0.9, 0.6, 0.3, 1.0)
bpy.data.images.new('MR', 4, 4)
bpy.data.images['MR'].generated_color = (1.0, 0.5, 0.25, 1.0)
bpy.data.node_groups.new('Pass', 'ShaderNodeTree')
bpy.data.node_groups['Pass'].interface.new_socket('Color', in_out='INPUT', socket_type='NodeSocketColor')
bpy.data.node_groups['Pass'].interface.new_socket('Color', in_out='OUTPUT', socket_type='NodeSocketColor')
bpy.data.node_groups['Pass'].nodes.new('NodeGroupInput')
bpy.data.node_groups['Pass'].nodes.new('NodeGroupOutput')
bpy.data.node_groups['Pass'].links.new(bpy.data.node_groups['Pass'].nodes['Group Input'].outputs[0], bpy.data.node_groups['Pass'].nodes['Group Output'].inputs[0])
bpy.data.node_groups.new('Split', 'ShaderNodeTree')
bpy.data.node_groups['Split'].interface.new_socket('Color', in_out='INPUT', socket_type='NodeSocketColor')
bpy.data.node_groups['Split'].interface.new_socket('Rough', in_out='OUTPUT', socket_type='NodeSocketFloat')
bpy.data.node_groups['Split'].interface.new_socket('Metal', in_out='OUTPUT', socket_type='NodeSocketFloat')
bpy.data.node_groups['Split'].nodes.new('NodeGroupInput')
bpy.data.node_groups['Split'].nodes.new('NodeGroupOutput')
bpy.data.node_groups['Split'].nodes.new('ShaderNodeSeparateColor')
bpy.data.node_groups['Split'].links.new(bpy.data.node_groups['Split'].nodes['Group Input'].outputs[0], bpy.data.node_groups['Split'].nodes['Separate Color'].inputs['Color'])
bpy.data.node_groups['Split'].links.new(bpy.data.node_groups['Split'].nodes['Separate Color'].outputs['Green'], bpy.data.node_groups['Split'].nodes['Group Output'].inputs['Rough'])
bpy.data.node_groups['Split'].links.new(bpy.data.node_groups['Split'].nodes['Separate Color'].outputs['Blue'], bpy.data.node_groups['Split'].nodes['Group Output'].inputs['Metal'])
bpy.data.node_groups.new('Dead', 'ShaderNodeTree')
bpy.data.node_groups['Dead'].interface.new_socket('Color', in_out='OUTPUT', socket_type='NodeSocketColor')
bpy.data.node_groups['Dead'].nodes.new('NodeGroupOutput')
bpy.data.node_groups['Dead'].nodes['Group Output'].inputs[0].default_value = (0.0, 1.0, 0.0, 1.0)
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Material'].node_tree.nodes['Image Texture'].image = bpy.data.images['Albedo']
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeGroup')
bpy.data.materials['Material'].node_tree.nodes['Group'].node_tree = bpy.data.node_groups['Pass']
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Group'].inputs[0])
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Group'].outputs[0], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Material'].node_tree.nodes['Image Texture.001'].image = bpy.data.images['MR']
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeGroup')
bpy.data.materials['Material'].node_tree.nodes['Group.001'].node_tree = bpy.data.node_groups['Split']
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Image Texture.001'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Group.001'].inputs[0])
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Group.001'].outputs['Rough'], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Roughness'])
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Group.001'].outputs['Metal'], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Metallic'])
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeGroup')
bpy.data.materials['Material'].node_tree.nodes['Group.002'].node_tree = bpy.data.node_groups['Dead']
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Group.002'].outputs[0], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Emission Color'])
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Emission Strength'].default_value = 1.0
bpy.ops.export_scene.gltf(filepath='out/unused.glb', export_unused_textures=True)
bpy.data.node_groups.new('glTF Material Output', 'ShaderNodeTree')
bpy.data.node_groups['glTF Material Output'].interface.new_socket('Occlusion', socket_type='NodeSocketFloat')
bpy.data.node_groups['glTF Material Output'].nodes.new('NodeGroupInput')
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeGroup')
bpy.data.materials['Material'].node_tree.nodes['Group.003'].node_tree = bpy.data.node_groups['glTF Material Output']
bpy.ops.export_scene.gltf(filepath='out/occ.glb')
