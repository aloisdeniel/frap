# Integration (WP 3.5 folding + WP 3.6 extensions): a hand-built 'glTF Material Output' group
# (iridescence factor and minimum on it; thickness 400.00001 and IOR 1.30005 on the BSDF), coat,
# anisotropy and sheen constants; a second material whose Coat Roughness is a Math ADD of its
# defaults (folded to 1, clearcoatRoughnessFactor 1) with transmission and IOR 1.2.
# golden: startup=factory tier=T2 compare=glb:out/m.glb glb_structure
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
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Coat Weight'].default_value = 1.0
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Coat Roughness'].default_value = 0.03
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Anisotropic'].default_value = 1.0
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Anisotropic Rotation'].default_value = -0.75
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Sheen Weight'].default_value = 1.0
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Sheen Tint'].default_value = (0, 0, 0, 1)
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeGroup')
bpy.data.materials['Material'].node_tree.nodes['Group'].node_tree = bpy.data.node_groups['glTF Material Output']
bpy.data.materials['Material'].node_tree.nodes['Group'].inputs['Iridescence Factor'].default_value = 0.5
bpy.data.materials['Material'].node_tree.nodes['Group'].inputs['Iridescence Thickness Minimum'].default_value = 300.0
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Thin Film Thickness'].default_value = 400.00001
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Thin Film IOR'].default_value = 1.30005
bpy.data.materials.new('U')
bpy.data.materials['U'].node_tree.nodes['Principled BSDF'].inputs['Transmission Weight'].default_value = 1.0
bpy.data.materials['U'].node_tree.nodes['Principled BSDF'].inputs['IOR'].default_value = 1.2
bpy.data.materials['U'].node_tree.nodes['Principled BSDF'].inputs['Coat Weight'].default_value = 0.5
bpy.data.materials['U'].node_tree.nodes.new('ShaderNodeMath')
bpy.data.materials['U'].node_tree.nodes['Math'].operation = 'ADD'
bpy.data.materials['U'].node_tree.links.new(bpy.data.materials['U'].node_tree.nodes['Math'].outputs[0], bpy.data.materials['U'].node_tree.nodes['Principled BSDF'].inputs['Coat Roughness'])
bpy.data.materials['U'].node_tree.nodes.new('ShaderNodeBsdfDiffuse')
bpy.ops.mesh.primitive_plane_add(location=(3, 0, 0))
bpy.context.object.data.materials.append(bpy.data.materials['U'])
bpy.ops.export_scene.gltf(filepath='out/m.glb')
