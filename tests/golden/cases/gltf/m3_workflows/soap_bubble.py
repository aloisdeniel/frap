# WP 3.9: a soap bubble. KHR_materials_iridescence (factor from the "glTF Material Output"
# group's Iridescence Factor, the thin-film thickness range from its Iridescence Thickness
# Minimum, which the add-on does not write without a thickness texture, and the Principled
# Thin Film Thickness as the maximum; the thin-film IOR 1.33) on a transmissive,
# rough-free material (KHR_materials_transmission); a beetle shell is iridescent but opaque.
# Round trip (§17.2).
# golden: startup=factory tier=T2 compare=glb:out/bubble.glb,glb:out/bubble_rt.glb
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
bpy.ops.mesh.primitive_uv_sphere_add(segments=12, ring_count=6, location=(0, 0, 1))
bpy.ops.object.shade_smooth()
bpy.data.objects['Sphere'].name = 'Bubble'
bpy.data.materials['Material'].name = 'Soap'
bpy.data.objects['Bubble'].data.materials.append(bpy.data.materials['Soap'])
bpy.data.materials['Soap'].node_tree.nodes['Principled BSDF'].inputs['Transmission Weight'].default_value = 1.0
bpy.data.materials['Soap'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.0
bpy.data.materials['Soap'].node_tree.nodes['Principled BSDF'].inputs['Thin Film Thickness'].default_value = 600.0
bpy.data.materials['Soap'].node_tree.nodes['Principled BSDF'].inputs['Thin Film IOR'].default_value = 1.33
bpy.data.materials['Soap'].node_tree.nodes.new('ShaderNodeGroup')
bpy.data.materials['Soap'].node_tree.nodes['Group'].node_tree = bpy.data.node_groups['glTF Material Output']
bpy.data.materials['Soap'].node_tree.nodes['Group'].inputs['Iridescence Factor'].default_value = 1.0
bpy.data.materials['Soap'].node_tree.nodes['Group'].inputs['Iridescence Thickness Minimum'].default_value = 200.0
bpy.ops.mesh.primitive_uv_sphere_add(segments=8, ring_count=4, radius=0.4, location=(2, 0, 0.4))
bpy.data.objects['Sphere'].name = 'Beetle'
bpy.data.materials.new('Shell')
bpy.data.objects['Beetle'].data.materials.append(bpy.data.materials['Shell'])
bpy.data.materials['Shell'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.05, 0.3, 0.1, 1.0)
bpy.data.materials['Shell'].node_tree.nodes['Principled BSDF'].inputs['Metallic'].default_value = 0.8
bpy.data.materials['Shell'].node_tree.nodes['Principled BSDF'].inputs['Thin Film Thickness'].default_value = 450.0
bpy.data.materials['Shell'].node_tree.nodes['Principled BSDF'].inputs['Thin Film IOR'].default_value = 1.6
bpy.data.materials['Shell'].node_tree.nodes.new('ShaderNodeGroup')
bpy.data.materials['Shell'].node_tree.nodes['Group'].node_tree = bpy.data.node_groups['glTF Material Output']
bpy.data.materials['Shell'].node_tree.nodes['Group'].inputs['Iridescence Factor'].default_value = 0.6
bpy.ops.export_scene.gltf(filepath='out/bubble.glb')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/bubble.glb')
bpy.ops.export_scene.gltf(filepath='out/bubble_rt.glb')
