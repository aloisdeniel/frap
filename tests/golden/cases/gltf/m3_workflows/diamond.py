# WP 3.9: a diamond ring. The stone combines KHR_materials_transmission, KHR_materials_volume
# (constant Thickness 0.8 from the "glTF Material Output" group, no Volume Absorption: infinite
# attenuation distance is not written), KHR_materials_ior 2.42 and KHR_materials_dispersion
# (Dispersion 0.4 from the group); the gold band is metallic. Round trip (§17.2).
# golden: startup=factory tier=T2 compare=glb:out/ring.glb,glb:out/ring_rt.glb
import bpy
bpy.data.objects.remove(bpy.data.objects['Cube'])
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
bpy.ops.mesh.primitive_cone_add(vertices=8, radius1=0.5, depth=0.6, location=(0, 0, 1.2), rotation=(3.1415927, 0, 0))
bpy.data.objects['Cone'].name = 'Stone'
bpy.data.materials['Material'].name = 'Diamond'
bpy.data.objects['Stone'].data.materials.append(bpy.data.materials['Diamond'])
bpy.data.materials['Diamond'].node_tree.nodes['Principled BSDF'].inputs['Transmission Weight'].default_value = 1.0
bpy.data.materials['Diamond'].node_tree.nodes['Principled BSDF'].inputs['IOR'].default_value = 2.42
bpy.data.materials['Diamond'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.0
bpy.data.materials['Diamond'].node_tree.nodes.new('ShaderNodeGroup')
bpy.data.materials['Diamond'].node_tree.nodes['Group'].node_tree = bpy.data.node_groups['glTF Material Output']
bpy.data.materials['Diamond'].node_tree.nodes['Group'].inputs['Thickness'].default_value = 0.8
bpy.data.materials['Diamond'].node_tree.nodes['Group'].inputs['Dispersion'].default_value = 0.4
bpy.ops.mesh.primitive_torus_add(major_segments=16, minor_segments=6, major_radius=0.8, minor_radius=0.1)
bpy.data.objects['Torus'].name = 'Band'
bpy.data.materials.new('Gold')
bpy.data.objects['Band'].data.materials.append(bpy.data.materials['Gold'])
bpy.data.materials['Gold'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (1.0, 0.77, 0.34, 1.0)
bpy.data.materials['Gold'].node_tree.nodes['Principled BSDF'].inputs['Metallic'].default_value = 1.0
bpy.data.materials['Gold'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.2
bpy.ops.export_scene.gltf(filepath='out/ring.glb')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/ring.glb')
bpy.ops.export_scene.gltf(filepath='out/ring_rt.glb')
