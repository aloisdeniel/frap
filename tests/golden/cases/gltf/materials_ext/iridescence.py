# WP 3.6: KHR_materials_iridescence export. The factor comes from the "glTF Material Output"
# group's Iridescence Factor, the maximum thickness and the thin-film IOR from the Principled
# BSDF (Blender's default 1.33 differs from glTF's 1.3, so it is written). The second material
# keeps Thin Film Thickness at 0, so the add-on drops its extension (__check_iridescence).
# golden: startup=factory tier=T2 compare=glb:out/iridescence.glb
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
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeGroup')
bpy.data.materials['Material'].node_tree.nodes['Group'].node_tree = bpy.data.node_groups['glTF Material Output']
bpy.data.materials['Material'].node_tree.nodes['Group'].inputs['Iridescence Factor'].default_value = 0.75
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Thin Film Thickness'].default_value = 550.0
bpy.data.materials.new('Bubble')
bpy.data.materials['Bubble'].node_tree.nodes.new('ShaderNodeGroup')
bpy.data.materials['Bubble'].node_tree.nodes['Group'].node_tree = bpy.data.node_groups['glTF Material Output']
bpy.data.materials['Bubble'].node_tree.nodes['Group'].inputs['Iridescence Factor'].default_value = 1.0
bpy.data.materials['Bubble'].node_tree.nodes['Principled BSDF'].inputs['Thin Film IOR'].default_value = 1.5
bpy.data.materials.new('Soap')
bpy.data.materials['Soap'].node_tree.nodes.new('ShaderNodeGroup')
bpy.data.materials['Soap'].node_tree.nodes['Group'].node_tree = bpy.data.node_groups['glTF Material Output']
bpy.data.materials['Soap'].node_tree.nodes['Group'].inputs['Iridescence Factor'].default_value = 1.0
bpy.data.materials['Soap'].node_tree.nodes['Principled BSDF'].inputs['Thin Film Thickness'].default_value = 400.0
bpy.data.materials['Soap'].node_tree.nodes['Principled BSDF'].inputs['Thin Film IOR'].default_value = 1.3
bpy.ops.mesh.primitive_plane_add(location=(3, 0, 0))
bpy.context.object.data.materials.append(bpy.data.materials['Bubble'])
bpy.ops.mesh.primitive_plane_add(location=(-3, 0, 0))
bpy.context.object.data.materials.append(bpy.data.materials['Soap'])
bpy.ops.export_scene.gltf(filepath='out/iridescence.glb')
