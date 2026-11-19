# WP 3.9: a weathered metal barrel with channel-packed textures of distinct pixels. A Separate
# Color node routes one "Mask" image's G channel to Roughness and its B channel to Metallic,
# a second image's R channel goes through Separate Color to the "glTF Material Output" group's
# Occlusion, and an emission texture with Emission Strength 1 gives emissiveTexture. The three
# images are 2x2, so occlusion and metallic-roughness are packed into one ORM PNG. Round trip
# (§17.2) of the export.
# golden: startup=factory tier=T2 compare=glb:out/barrel.glb,glb:out/barrel_rt.glb
import bpy
bpy.ops.mesh.primitive_cylinder_add(vertices=12, radius=0.8, depth=2, location=(0, 0, 1))
bpy.data.objects['Cylinder'].name = 'Barrel'
bpy.data.objects.remove(bpy.data.objects['Cube'])
bpy.data.objects['Barrel'].data.materials.append(bpy.data.materials['Material'])
bpy.data.materials['Material'].name = 'Rust'
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
bpy.data.images.new('Mask', 2, 2)
bpy.data.images['Mask'].colorspace_settings.name = 'Non-Color'
bpy.data.images['Mask'].pixels = [0.9, 0.1, 0.2, 1.0, 0.1, 0.8, 0.9, 1.0, 0.5, 0.3, 0.6, 1.0, 0.2, 0.6, 0.0, 1.0]
bpy.data.images.new('Occ', 2, 2)
bpy.data.images['Occ'].colorspace_settings.name = 'Non-Color'
bpy.data.images['Occ'].pixels = [1.0, 0.0, 0.0, 1.0, 0.7, 0.0, 0.0, 1.0, 0.4, 0.0, 0.0, 1.0, 0.2, 0.0, 0.0, 1.0]
bpy.data.images.new('Glow', 2, 2)
bpy.data.images['Glow'].pixels = [0.0, 0.0, 0.0, 1.0, 1.0, 0.3, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.8, 1.0, 1.0]
bpy.data.materials['Rust'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Rust'].node_tree.nodes['Image Texture'].image = bpy.data.images['Mask']
bpy.data.materials['Rust'].node_tree.nodes.new('ShaderNodeSeparateColor')
bpy.data.materials['Rust'].node_tree.links.new(bpy.data.materials['Rust'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Rust'].node_tree.nodes['Separate Color'].inputs['Color'])
bpy.data.materials['Rust'].node_tree.links.new(bpy.data.materials['Rust'].node_tree.nodes['Separate Color'].outputs['Green'], bpy.data.materials['Rust'].node_tree.nodes['Principled BSDF'].inputs['Roughness'])
bpy.data.materials['Rust'].node_tree.links.new(bpy.data.materials['Rust'].node_tree.nodes['Separate Color'].outputs['Blue'], bpy.data.materials['Rust'].node_tree.nodes['Principled BSDF'].inputs['Metallic'])
bpy.data.materials['Rust'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Rust'].node_tree.nodes['Image Texture.001'].image = bpy.data.images['Occ']
bpy.data.materials['Rust'].node_tree.nodes.new('ShaderNodeSeparateColor')
bpy.data.materials['Rust'].node_tree.links.new(bpy.data.materials['Rust'].node_tree.nodes['Image Texture.001'].outputs['Color'], bpy.data.materials['Rust'].node_tree.nodes['Separate Color.001'].inputs['Color'])
bpy.data.materials['Rust'].node_tree.nodes.new('ShaderNodeGroup')
bpy.data.materials['Rust'].node_tree.nodes['Group'].node_tree = bpy.data.node_groups['glTF Material Output']
bpy.data.materials['Rust'].node_tree.links.new(bpy.data.materials['Rust'].node_tree.nodes['Separate Color.001'].outputs['Red'], bpy.data.materials['Rust'].node_tree.nodes['Group'].inputs['Occlusion'])
bpy.data.materials['Rust'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Rust'].node_tree.nodes['Image Texture.002'].image = bpy.data.images['Glow']
bpy.data.materials['Rust'].node_tree.links.new(bpy.data.materials['Rust'].node_tree.nodes['Image Texture.002'].outputs['Color'], bpy.data.materials['Rust'].node_tree.nodes['Principled BSDF'].inputs['Emission Color'])
bpy.data.materials['Rust'].node_tree.nodes['Principled BSDF'].inputs['Emission Strength'].default_value = 1.0
bpy.data.materials['Rust'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.5, 0.3, 0.2, 1.0)
bpy.ops.export_scene.gltf(filepath='out/barrel.glb')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/barrel.glb')
bpy.ops.export_scene.gltf(filepath='out/barrel_rt.glb')
