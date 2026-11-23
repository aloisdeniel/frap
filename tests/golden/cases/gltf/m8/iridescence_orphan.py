# WP 8.12: a `KHR_materials_iridescence` texture whose extension is dropped. The glTF Material
# Output's `Iridescence Factor` reads an image through Separate Color (no constant factor), so
# `iridescenceFactor` is unset and `__check_iridescence` removes the extension: its texture,
# sampler and image are not written either (the glTF has no `textures`, `samplers` or
# `images`), with and without pointer animation. A second material keeps the same kind of
# texture on a Principled `Base Color`, which is written.
# golden: startup=factory tier=T2 compare=glb:out/orphan.glb,glb:out/kept.glb glb_structure
import bpy
bpy.data.node_groups.new('glTF Material Output', 'ShaderNodeTree')
bpy.data.node_groups['glTF Material Output'].interface.new_socket('Iridescence Factor', socket_type='NodeSocketFloat')
bpy.data.node_groups['glTF Material Output'].interface.items_tree['Iridescence Factor'].default_value = 0.0
bpy.data.node_groups['glTF Material Output'].interface.new_socket('Iridescence Thickness Minimum', socket_type='NodeSocketFloat')
bpy.data.node_groups['glTF Material Output'].interface.items_tree['Iridescence Thickness Minimum'].default_value = 100.0
bpy.data.node_groups['glTF Material Output'].nodes.new('NodeGroupOutput')
bpy.data.node_groups['glTF Material Output'].nodes.new('NodeGroupInput')
bpy.data.images.new('Film', 4, 4)
bpy.data.images['Film'].generated_color = (0.7, 0.3, 0.1, 1.0)
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeGroup')
bpy.data.materials['Material'].node_tree.nodes['Group'].node_tree = bpy.data.node_groups['glTF Material Output']
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Thin Film Thickness'].default_value = 550.0
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Material'].node_tree.nodes['Image Texture'].image = bpy.data.images['Film']
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeSeparateColor')
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Separate Color'].inputs['Color'])
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Separate Color'].outputs['Red'], bpy.data.materials['Material'].node_tree.nodes['Group'].inputs['Iridescence Factor'])
bpy.ops.export_scene.gltf(filepath='out/orphan.glb')
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.ops.export_scene.gltf(filepath='out/kept.glb')
