# WP 3.9: a material library sheet: one sphere per material in a single file, covering every
# KHR_materials_* extension frap exports at once (clearcoat, sheen, transmission, volume, ior,
# specular, iridescence, anisotropy, dispersion, emissive_strength, unlit), so extensionsUsed
# lists them all and each material keeps only its own. Round trip (§17.2): the importer raises
# every extension back into node graphs and the re-export lists the same extensions.
# golden: startup=empty tier=T2 compare=glb:out/library.glb,glb:out/library_rt.glb
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
bpy.ops.mesh.primitive_uv_sphere_add(segments=8, ring_count=4, location=(0, 0, 0))
bpy.context.object.name = 'S_Coat'
bpy.data.materials.new('Coat')
bpy.context.object.data.materials.append(bpy.data.materials['Coat'])
bpy.data.materials['Coat'].node_tree.nodes['Principled BSDF'].inputs['Coat Weight'].default_value = 0.9
bpy.data.materials['Coat'].node_tree.nodes['Principled BSDF'].inputs['Coat Roughness'].default_value = 0.1
bpy.ops.mesh.primitive_uv_sphere_add(segments=8, ring_count=4, location=(3, 0, 0))
bpy.context.object.name = 'S_Sheen'
bpy.data.materials.new('Sheen')
bpy.context.object.data.materials.append(bpy.data.materials['Sheen'])
bpy.data.materials['Sheen'].node_tree.nodes['Principled BSDF'].inputs['Sheen Weight'].default_value = 1.0
bpy.data.materials['Sheen'].node_tree.nodes['Principled BSDF'].inputs['Sheen Tint'].default_value = (0.3, 0.6, 0.9, 1.0)
bpy.ops.mesh.primitive_uv_sphere_add(segments=8, ring_count=4, location=(6, 0, 0))
bpy.context.object.name = 'S_Glass'
bpy.data.materials.new('Glass')
bpy.context.object.data.materials.append(bpy.data.materials['Glass'])
bpy.data.materials['Glass'].node_tree.nodes['Principled BSDF'].inputs['Transmission Weight'].default_value = 1.0
bpy.data.materials['Glass'].node_tree.nodes['Principled BSDF'].inputs['IOR'].default_value = 1.6
bpy.data.materials['Glass'].node_tree.nodes.new('ShaderNodeGroup')
bpy.data.materials['Glass'].node_tree.nodes['Group'].node_tree = bpy.data.node_groups['glTF Material Output']
bpy.data.materials['Glass'].node_tree.nodes['Group'].inputs['Thickness'].default_value = 0.3
bpy.data.materials['Glass'].node_tree.nodes['Group'].inputs['Dispersion'].default_value = 0.2
bpy.data.materials['Glass'].node_tree.nodes.new('ShaderNodeVolumeAbsorption')
bpy.data.materials['Glass'].node_tree.nodes['Volume Absorption'].inputs['Color'].default_value = (0.5, 0.8, 1.0, 1.0)
bpy.data.materials['Glass'].node_tree.nodes['Volume Absorption'].inputs['Density'].default_value = 0.5
bpy.data.materials['Glass'].node_tree.links.new(bpy.data.materials['Glass'].node_tree.nodes['Volume Absorption'].outputs['Volume'], bpy.data.materials['Glass'].node_tree.nodes['Material Output'].inputs['Volume'])
bpy.ops.mesh.primitive_uv_sphere_add(segments=8, ring_count=4, location=(0, 3, 0))
bpy.context.object.name = 'S_Spec'
bpy.data.materials.new('Spec')
bpy.context.object.data.materials.append(bpy.data.materials['Spec'])
bpy.data.materials['Spec'].node_tree.nodes['Principled BSDF'].inputs['Specular IOR Level'].default_value = 0.7
bpy.data.materials['Spec'].node_tree.nodes['Principled BSDF'].inputs['Specular Tint'].default_value = (1.0, 0.8, 0.6, 1.0)
bpy.ops.mesh.primitive_uv_sphere_add(segments=8, ring_count=4, location=(3, 3, 0))
bpy.context.object.name = 'S_Film'
bpy.data.materials.new('Film')
bpy.context.object.data.materials.append(bpy.data.materials['Film'])
bpy.data.materials['Film'].node_tree.nodes['Principled BSDF'].inputs['Thin Film Thickness'].default_value = 500.0
bpy.data.materials['Film'].node_tree.nodes.new('ShaderNodeGroup')
bpy.data.materials['Film'].node_tree.nodes['Group'].node_tree = bpy.data.node_groups['glTF Material Output']
bpy.data.materials['Film'].node_tree.nodes['Group'].inputs['Iridescence Factor'].default_value = 0.8
bpy.ops.mesh.primitive_uv_sphere_add(segments=8, ring_count=4, location=(6, 3, 0))
bpy.context.object.name = 'S_Aniso'
bpy.data.materials.new('Aniso')
bpy.context.object.data.materials.append(bpy.data.materials['Aniso'])
bpy.data.materials['Aniso'].node_tree.nodes['Principled BSDF'].inputs['Metallic'].default_value = 1.0
bpy.data.materials['Aniso'].node_tree.nodes['Principled BSDF'].inputs['Anisotropic'].default_value = 0.4
bpy.data.materials['Aniso'].node_tree.nodes['Principled BSDF'].inputs['Anisotropic Rotation'].default_value = 0.1
bpy.ops.mesh.primitive_uv_sphere_add(segments=8, ring_count=4, location=(0, 6, 0))
bpy.context.object.name = 'S_Glow'
bpy.data.materials.new('Glow')
bpy.context.object.data.materials.append(bpy.data.materials['Glow'])
bpy.data.materials['Glow'].node_tree.nodes['Principled BSDF'].inputs['Emission Color'].default_value = (0.2, 1.0, 0.4, 1.0)
bpy.data.materials['Glow'].node_tree.nodes['Principled BSDF'].inputs['Emission Strength'].default_value = 3.0
bpy.ops.mesh.primitive_uv_sphere_add(segments=8, ring_count=4, location=(3, 6, 0))
bpy.context.object.name = 'S_Flat'
bpy.data.materials.new('Flat')
bpy.context.object.data.materials.append(bpy.data.materials['Flat'])
bpy.data.materials['Flat'].node_tree.nodes.new('ShaderNodeBackground')
bpy.data.materials['Flat'].node_tree.nodes['Background'].inputs['Color'].default_value = (0.9, 0.2, 0.6, 1.0)
bpy.data.materials['Flat'].node_tree.links.new(bpy.data.materials['Flat'].node_tree.nodes['Background'].outputs[0], bpy.data.materials['Flat'].node_tree.nodes['Material Output'].inputs['Surface'])
bpy.ops.export_scene.gltf(filepath='out/library.glb')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/library.glb')
bpy.ops.export_scene.gltf(filepath='out/library_rt.glb')
