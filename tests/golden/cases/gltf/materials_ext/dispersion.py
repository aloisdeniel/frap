# WP 3.6: KHR_materials_dispersion export from the "glTF Material Output" group's Dispersion:
# kept with a volume and transmission (first material), dropped with the volume when there is no
# transmission (second material, __check_volume then __check_dispersion).
# golden: startup=factory tier=T2 compare=glb:out/dispersion.glb
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
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Transmission Weight'].default_value = 1.0
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeGroup')
bpy.data.materials['Material'].node_tree.nodes['Group'].node_tree = bpy.data.node_groups['glTF Material Output']
bpy.data.materials['Material'].node_tree.nodes['Group'].inputs['Thickness'].default_value = 1.0
bpy.data.materials['Material'].node_tree.nodes['Group'].inputs['Dispersion'].default_value = 0.3
bpy.data.materials.new('Prism')
bpy.data.materials['Prism'].node_tree.nodes.new('ShaderNodeGroup')
bpy.data.materials['Prism'].node_tree.nodes['Group'].node_tree = bpy.data.node_groups['glTF Material Output']
bpy.data.materials['Prism'].node_tree.nodes['Group'].inputs['Thickness'].default_value = 1.0
bpy.data.materials['Prism'].node_tree.nodes['Group'].inputs['Dispersion'].default_value = 0.3
bpy.ops.mesh.primitive_plane_add(location=(3, 0, 0))
bpy.context.object.data.materials.append(bpy.data.materials['Prism'])
bpy.ops.export_scene.gltf(filepath='out/dispersion.glb')
