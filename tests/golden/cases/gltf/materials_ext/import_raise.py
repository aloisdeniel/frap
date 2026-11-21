# WP 3.6: import raise round-trip of every KHR_materials_* extension. Each tool exports its own
# file, re-imports it into an empty file and exports again: the raised node graphs (Principled
# BSDF sockets, the "glTF Material Output" settings node, the Volume Absorption node) compare as
# materials, and the second export canonically. The thin-film maximum comes back as 400: the
# add-on's importer reads it from the key `iridescenceThickness`.
# golden: startup=factory tier=T2 compare=materials,glb:out/again.glb glb_structure
import bpy
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Coat Weight'].default_value = 0.8
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Coat Roughness'].default_value = 0.2
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Transmission Weight'].default_value = 0.6
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['IOR'].default_value = 1.45
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Specular IOR Level'].default_value = 0.3
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Specular Tint'].default_value = (0.5, 0.6, 0.7, 1)
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Sheen Weight'].default_value = 1.0
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Sheen Tint'].default_value = (0.2, 0.3, 0.4, 1)
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Sheen Roughness'].default_value = 0.25
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Anisotropic'].default_value = 0.5
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Anisotropic Rotation'].default_value = 0.25
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
bpy.data.materials.new('Glass')
bpy.data.materials['Glass'].node_tree.nodes['Principled BSDF'].inputs['Transmission Weight'].default_value = 1.0
bpy.data.materials['Glass'].node_tree.nodes['Principled BSDF'].inputs['Thin Film Thickness'].default_value = 550.0
bpy.data.materials['Glass'].node_tree.nodes['Principled BSDF'].inputs['Thin Film IOR'].default_value = 1.5
bpy.data.materials['Glass'].node_tree.nodes.new('ShaderNodeGroup')
bpy.data.materials['Glass'].node_tree.nodes['Group'].node_tree = bpy.data.node_groups['glTF Material Output']
bpy.data.materials['Glass'].node_tree.nodes['Group'].inputs['Thickness'].default_value = 0.5
bpy.data.materials['Glass'].node_tree.nodes['Group'].inputs['Dispersion'].default_value = 0.25
bpy.data.materials['Glass'].node_tree.nodes['Group'].inputs['Iridescence Factor'].default_value = 0.75
bpy.data.materials['Glass'].node_tree.nodes['Group'].inputs['Iridescence Thickness Minimum'].default_value = 150.0
bpy.data.materials['Glass'].node_tree.nodes.new('ShaderNodeVolumeAbsorption')
bpy.data.materials['Glass'].node_tree.nodes['Volume Absorption'].inputs['Color'].default_value = (0.9, 0.5, 0.25, 1)
bpy.data.materials['Glass'].node_tree.nodes['Volume Absorption'].inputs['Density'].default_value = 2.0
bpy.data.materials['Glass'].node_tree.links.new(bpy.data.materials['Glass'].node_tree.nodes['Volume Absorption'].outputs['Volume'], bpy.data.materials['Glass'].node_tree.nodes['Material Output'].inputs['Volume'])
bpy.ops.mesh.primitive_plane_add(location=(3, 0, 0))
bpy.context.object.data.materials.append(bpy.data.materials['Glass'])
bpy.ops.export_scene.gltf(filepath='out/first.glb')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/first.glb')
bpy.ops.export_scene.gltf(filepath='out/again.glb')
