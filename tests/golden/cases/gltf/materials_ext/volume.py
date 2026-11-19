# WP 3.6: KHR_materials_volume export. The add-on's "glTF Material Output" group (written out as
# create_settings_group does) carries the Thickness; a Volume Absorption node on the Material
# Output's Volume gives attenuationColor and attenuationDistance (1 / Density). With
# transmission the volume is kept and the material becomes single-sided; without transmission
# (second material) the volume is dropped, but the material still loses doubleSided and its
# IOR is kept (__check_ior runs before __check_volume).
# golden: startup=factory tier=T2 compare=glb:out/volume.glb
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
bpy.data.materials['Material'].node_tree.nodes['Group'].inputs['Thickness'].default_value = 0.25
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeVolumeAbsorption')
bpy.data.materials['Material'].node_tree.nodes['Volume Absorption'].inputs['Color'].default_value = (0.9, 0.5, 0.25, 1)
bpy.data.materials['Material'].node_tree.nodes['Volume Absorption'].inputs['Density'].default_value = 4.0
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Volume Absorption'].outputs['Volume'], bpy.data.materials['Material'].node_tree.nodes['Material Output'].inputs['Volume'])
bpy.data.materials.new('Thick')
bpy.data.materials['Thick'].node_tree.nodes['Principled BSDF'].inputs['IOR'].default_value = 1.33
bpy.data.materials['Thick'].node_tree.nodes.new('ShaderNodeGroup')
bpy.data.materials['Thick'].node_tree.nodes['Group'].node_tree = bpy.data.node_groups['glTF Material Output']
bpy.data.materials['Thick'].node_tree.nodes['Group'].inputs['Thickness'].default_value = 2.0
bpy.ops.mesh.primitive_plane_add(location=(3, 0, 0))
bpy.context.object.data.materials.append(bpy.data.materials['Thick'])
bpy.ops.export_scene.gltf(filepath='out/volume.glb')
