# WP 3.5: the exporter reads the inlined tree, where links from constants are folded: a Mix RGBA
# of constants on Base Color → (0.5, 0, 0.5); a Map Range of 0.25 → roughness 0.25; a muted Math
# with unlinked inputs leaves Metallic its own value (0); a Normal Map Strength from a Value node
# (3) → normalTexture.scale 3; a Mapping Location from a Combine XYZ (X 0.5) → offset [0.5, 0].
# A Math cycle on Base Color and a Reroute loop on Alpha (invalid links) export without crashing,
# the cycle's Image Texture is not exported.
# golden: startup=factory tier=T2 compare=glb:out/fold.glb glb_structure
import bpy
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeMix')
bpy.data.materials['Material'].node_tree.nodes['Mix'].data_type = 'RGBA'
bpy.data.materials['Material'].node_tree.nodes['Mix'].inputs['Factor'].default_value = 0.5
bpy.data.materials['Material'].node_tree.nodes['Mix'].inputs['A'].default_value = (1.0, 0.0, 0.0, 1.0)
bpy.data.materials['Material'].node_tree.nodes['Mix'].inputs['B'].default_value = (0.0, 0.0, 1.0, 1.0)
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Mix'].outputs['Result'], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeMapRange')
bpy.data.materials['Material'].node_tree.nodes['Map Range'].inputs['Value'].default_value = 0.25
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Map Range'].outputs['Result'], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Roughness'])
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeMath')
bpy.data.materials['Material'].node_tree.nodes['Math'].operation = 'MULTIPLY'
bpy.data.materials['Material'].node_tree.nodes['Math'].inputs[0].default_value = 0.5
bpy.data.materials['Material'].node_tree.nodes['Math'].inputs[1].default_value = 0.8
bpy.data.materials['Material'].node_tree.nodes['Math'].mute = True
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Math'].outputs[0], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Metallic'])
bpy.data.images.new('T', 4, 4)
bpy.data.images['T'].generated_color = (0.9, 0.6, 0.3, 1.0)
bpy.data.meshes['Cube'].materials.append(bpy.data.materials.new('Bump'))
bpy.data.materials['Bump'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Bump'].node_tree.nodes['Image Texture'].image = bpy.data.images['T']
bpy.data.materials['Bump'].node_tree.nodes.new('ShaderNodeNormalMap')
bpy.data.materials['Bump'].node_tree.nodes.new('ShaderNodeValue')
bpy.data.materials['Bump'].node_tree.nodes['Value'].outputs[0].default_value = 3.0
bpy.data.materials['Bump'].node_tree.links.new(bpy.data.materials['Bump'].node_tree.nodes['Value'].outputs[0], bpy.data.materials['Bump'].node_tree.nodes['Normal Map'].inputs['Strength'])
bpy.data.materials['Bump'].node_tree.links.new(bpy.data.materials['Bump'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Bump'].node_tree.nodes['Normal Map'].inputs['Color'])
bpy.data.materials['Bump'].node_tree.links.new(bpy.data.materials['Bump'].node_tree.nodes['Normal Map'].outputs['Normal'], bpy.data.materials['Bump'].node_tree.nodes['Principled BSDF'].inputs['Normal'])
bpy.data.meshes['Cube'].materials.append(bpy.data.materials.new('Linked'))
bpy.data.materials['Linked'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Linked'].node_tree.nodes['Image Texture'].image = bpy.data.images['T']
bpy.data.materials['Linked'].node_tree.nodes.new('ShaderNodeMapping')
bpy.data.materials['Linked'].node_tree.nodes.new('ShaderNodeCombineXYZ')
bpy.data.materials['Linked'].node_tree.nodes['Combine XYZ'].inputs['X'].default_value = 0.5
bpy.data.materials['Linked'].node_tree.links.new(bpy.data.materials['Linked'].node_tree.nodes['Combine XYZ'].outputs[0], bpy.data.materials['Linked'].node_tree.nodes['Mapping'].inputs['Location'])
bpy.data.materials['Linked'].node_tree.links.new(bpy.data.materials['Linked'].node_tree.nodes['Mapping'].outputs['Vector'], bpy.data.materials['Linked'].node_tree.nodes['Image Texture'].inputs['Vector'])
bpy.data.materials['Linked'].node_tree.links.new(bpy.data.materials['Linked'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Linked'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.data.meshes['Cube'].materials.append(bpy.data.materials.new('Cycle'))
bpy.data.materials['Cycle'].node_tree.nodes.new('ShaderNodeMath')
bpy.data.materials['Cycle'].node_tree.nodes.new('ShaderNodeMath')
bpy.data.materials['Cycle'].node_tree.links.new(bpy.data.materials['Cycle'].node_tree.nodes['Math'].outputs[0], bpy.data.materials['Cycle'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.data.materials['Cycle'].node_tree.links.new(bpy.data.materials['Cycle'].node_tree.nodes['Math.001'].outputs[0], bpy.data.materials['Cycle'].node_tree.nodes['Math'].inputs[0])
bpy.data.materials['Cycle'].node_tree.links.new(bpy.data.materials['Cycle'].node_tree.nodes['Math'].outputs[0], bpy.data.materials['Cycle'].node_tree.nodes['Math.001'].inputs[0])
bpy.data.materials['Cycle'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Cycle'].node_tree.nodes['Image Texture'].image = bpy.data.images['T']
bpy.data.materials['Cycle'].node_tree.links.new(bpy.data.materials['Cycle'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Cycle'].node_tree.nodes['Math.001'].inputs[1])
bpy.data.materials['Cycle'].node_tree.nodes.new('NodeReroute')
bpy.data.materials['Cycle'].node_tree.nodes.new('NodeReroute')
bpy.data.materials['Cycle'].node_tree.links.new(bpy.data.materials['Cycle'].node_tree.nodes['Reroute'].outputs[0], bpy.data.materials['Cycle'].node_tree.nodes['Reroute.001'].inputs[0])
bpy.data.materials['Cycle'].node_tree.links.new(bpy.data.materials['Cycle'].node_tree.nodes['Reroute.001'].outputs[0], bpy.data.materials['Cycle'].node_tree.nodes['Reroute'].inputs[0])
bpy.data.materials['Cycle'].node_tree.links.new(bpy.data.materials['Cycle'].node_tree.nodes['Reroute'].outputs[0], bpy.data.materials['Cycle'].node_tree.nodes['Principled BSDF'].inputs['Alpha'])
bpy.data.meshes['Cube'].polygons[1].material_index = 1
bpy.data.meshes['Cube'].polygons[2].material_index = 2
bpy.data.meshes['Cube'].polygons[3].material_index = 3
bpy.ops.export_scene.gltf(filepath='out/fold.glb')
