# WP 3.5: import of the core material features, raised as the add-on's importer builds them:
# the "glTF Material Output" group node with an occlusion texture through an "Occlusion
# Strength" Mix, a normal map with its strength, an alpha-clipped (MASK 0.4) base colour
# texture with a KHR_texture_transform Mapping node, emission with
# KHR_materials_emissive_strength, and a KHR_materials_unlit material (Light Path trick). Each
# tool imports its own export.
# golden: startup=factory tier=T1 compare=materials tol=1e-5
import bpy
bpy.data.node_groups.new('glTF Material Output', 'ShaderNodeTree')
bpy.data.node_groups['glTF Material Output'].interface.new_socket('Occlusion', socket_type='NodeSocketFloat')
bpy.data.node_groups['glTF Material Output'].nodes.new('NodeGroupOutput')
bpy.data.node_groups['glTF Material Output'].nodes.new('NodeGroupInput')
bpy.data.images.new('Occ', 2, 2)
bpy.data.images['Occ'].generated_color = (0.25, 0.5, 0.75, 1.0)
bpy.data.images.new('Bumps', 2, 2)
bpy.data.images['Bumps'].generated_color = (0.5, 0.5, 1.0, 1.0)
bpy.data.images.new('Leaf', 2, 2, alpha=True)
bpy.data.images['Leaf'].pixels = [1.0, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.5, 0.0, 0.0, 1.0, 0.25, 1.0, 1.0, 1.0, 0.0]
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Material'].node_tree.nodes['Image Texture'].image = bpy.data.images['Occ']
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeGroup')
bpy.data.materials['Material'].node_tree.nodes['Group'].node_tree = bpy.data.node_groups['glTF Material Output']
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeMix')
bpy.data.materials['Material'].node_tree.nodes['Mix'].data_type = 'RGBA'
bpy.data.materials['Material'].node_tree.nodes['Mix'].inputs['Factor'].default_value = 0.6
bpy.data.materials['Material'].node_tree.nodes['Mix'].inputs['A'].default_value = (1.0, 1.0, 1.0, 1.0)
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Mix'].inputs['B'])
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Mix'].outputs['Result'], bpy.data.materials['Material'].node_tree.nodes['Group'].inputs['Occlusion'])
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Material'].node_tree.nodes['Image Texture.001'].image = bpy.data.images['Bumps']
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeNormalMap')
bpy.data.materials['Material'].node_tree.nodes['Normal Map'].inputs['Strength'].default_value = 0.7
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Image Texture.001'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Normal Map'].inputs['Color'])
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Normal Map'].outputs['Normal'], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Normal'])
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Emission Color'].default_value = (1.0, 0.5, 0.2, 1.0)
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Emission Strength'].default_value = 4.0
bpy.data.meshes['Cube'].materials.append(bpy.data.materials.new('Leaf'))
bpy.data.materials['Leaf'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Leaf'].node_tree.nodes['Image Texture'].image = bpy.data.images['Leaf']
bpy.data.materials['Leaf'].node_tree.nodes.new('ShaderNodeMapping')
bpy.data.materials['Leaf'].node_tree.nodes['Mapping'].inputs['Location'].default_value = (0.1, 0.2, 0.0)
bpy.data.materials['Leaf'].node_tree.nodes['Mapping'].inputs['Scale'].default_value = (2.0, 2.0, 1.0)
bpy.data.materials['Leaf'].node_tree.links.new(bpy.data.materials['Leaf'].node_tree.nodes['Mapping'].outputs['Vector'], bpy.data.materials['Leaf'].node_tree.nodes['Image Texture'].inputs['Vector'])
bpy.data.materials['Leaf'].node_tree.nodes.new('ShaderNodeMath')
bpy.data.materials['Leaf'].node_tree.nodes.new('ShaderNodeMath')
bpy.data.materials['Leaf'].node_tree.nodes['Math'].operation = 'SUBTRACT'
bpy.data.materials['Leaf'].node_tree.nodes['Math'].inputs[0].default_value = 1.0
bpy.data.materials['Leaf'].node_tree.nodes['Math.001'].operation = 'LESS_THAN'
bpy.data.materials['Leaf'].node_tree.nodes['Math.001'].inputs[1].default_value = 0.4
bpy.data.materials['Leaf'].node_tree.links.new(bpy.data.materials['Leaf'].node_tree.nodes['Image Texture'].outputs['Alpha'], bpy.data.materials['Leaf'].node_tree.nodes['Math.001'].inputs[0])
bpy.data.materials['Leaf'].node_tree.links.new(bpy.data.materials['Leaf'].node_tree.nodes['Math.001'].outputs[0], bpy.data.materials['Leaf'].node_tree.nodes['Math'].inputs[1])
bpy.data.materials['Leaf'].node_tree.links.new(bpy.data.materials['Leaf'].node_tree.nodes['Math'].outputs[0], bpy.data.materials['Leaf'].node_tree.nodes['Principled BSDF'].inputs['Alpha'])
bpy.data.materials['Leaf'].node_tree.links.new(bpy.data.materials['Leaf'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Leaf'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.data.meshes['Cube'].materials.append(bpy.data.materials.new('Flat'))
bpy.data.materials['Flat'].node_tree.nodes.new('ShaderNodeBackground')
bpy.data.materials['Flat'].node_tree.nodes['Background'].inputs['Color'].default_value = (0.2, 0.4, 0.6, 1.0)
bpy.data.materials['Flat'].node_tree.links.new(bpy.data.materials['Flat'].node_tree.nodes['Background'].outputs[0], bpy.data.materials['Flat'].node_tree.nodes['Material Output'].inputs['Surface'])
bpy.data.meshes['Cube'].polygons[1].material_index = 1
bpy.data.meshes['Cube'].polygons[2].material_index = 2
bpy.ops.export_scene.gltf(filepath='out/core.glb')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/core.glb')
