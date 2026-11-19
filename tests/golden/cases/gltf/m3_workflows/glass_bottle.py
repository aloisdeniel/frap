# WP 3.9: a glass bottle. KHR_materials_transmission (weight 1) with KHR_materials_volume: the
# "glTF Material Output" group's Thickness is read from the G channel (Separate Color) of an
# image of distinct pixels (thicknessTexture), a Volume Absorption node gives attenuationColor
# and attenuationDistance (1 / Density); KHR_materials_ior 1.52 accompanies them, and the
# volume makes the material single-sided. The cork is plain. Round trip (§17.2).
# golden: startup=factory tier=T2 compare=glb:out/bottle.glb,glb:out/bottle_rt.glb
import bpy
bpy.data.objects.remove(bpy.data.objects['Cube'])
bpy.ops.mesh.primitive_cylinder_add(vertices=16, radius=0.4, depth=1.5, location=(0, 0, 0.75))
bpy.data.objects['Cylinder'].name = 'Bottle'
bpy.ops.object.shade_smooth()
bpy.data.materials['Material'].name = 'Glass'
bpy.data.objects['Bottle'].data.materials.append(bpy.data.materials['Glass'])
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
bpy.data.materials['Glass'].node_tree.nodes['Principled BSDF'].inputs['Transmission Weight'].default_value = 1.0
bpy.data.materials['Glass'].node_tree.nodes['Principled BSDF'].inputs['IOR'].default_value = 1.52
bpy.data.materials['Glass'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.05
bpy.data.materials['Glass'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.8, 1.0, 0.85, 1.0)
bpy.data.images.new('Wall', 2, 2)
bpy.data.images['Wall'].colorspace_settings.name = 'Non-Color'
bpy.data.images['Wall'].pixels = [0.0, 0.2, 0.9, 1.0, 0.1, 0.5, 0.8, 1.0, 0.2, 0.8, 0.7, 1.0, 0.3, 1.0, 0.6, 1.0]
bpy.data.materials['Glass'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Glass'].node_tree.nodes['Image Texture'].image = bpy.data.images['Wall']
bpy.data.materials['Glass'].node_tree.nodes.new('ShaderNodeSeparateColor')
bpy.data.materials['Glass'].node_tree.links.new(bpy.data.materials['Glass'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Glass'].node_tree.nodes['Separate Color'].inputs['Color'])
bpy.data.materials['Glass'].node_tree.nodes.new('ShaderNodeGroup')
bpy.data.materials['Glass'].node_tree.nodes['Group'].node_tree = bpy.data.node_groups['glTF Material Output']
bpy.data.materials['Glass'].node_tree.links.new(bpy.data.materials['Glass'].node_tree.nodes['Separate Color'].outputs['Green'], bpy.data.materials['Glass'].node_tree.nodes['Group'].inputs['Thickness'])
bpy.data.materials['Glass'].node_tree.nodes.new('ShaderNodeVolumeAbsorption')
bpy.data.materials['Glass'].node_tree.nodes['Volume Absorption'].inputs['Color'].default_value = (0.6, 0.9, 0.7, 1.0)
bpy.data.materials['Glass'].node_tree.nodes['Volume Absorption'].inputs['Density'].default_value = 2.5
bpy.data.materials['Glass'].node_tree.links.new(bpy.data.materials['Glass'].node_tree.nodes['Volume Absorption'].outputs['Volume'], bpy.data.materials['Glass'].node_tree.nodes['Material Output'].inputs['Volume'])
bpy.ops.mesh.primitive_cylinder_add(vertices=8, radius=0.12, depth=0.2, location=(0, 0, 1.6))
bpy.data.objects['Cylinder'].name = 'Cork'
bpy.data.materials.new('Cork')
bpy.data.objects['Cork'].data.materials.append(bpy.data.materials['Cork'])
bpy.data.materials['Cork'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.6, 0.4, 0.2, 1.0)
bpy.data.materials['Cork'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.9
bpy.ops.export_scene.gltf(filepath='out/bottle.glb')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/bottle.glb')
bpy.ops.export_scene.gltf(filepath='out/bottle_rt.glb')
