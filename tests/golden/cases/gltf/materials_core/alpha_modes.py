# WP 3.5: the alpha mode comes from the node graph, never from `blend_method`: a constant 0.5
# → BLEND; Math ROUND of 0.7 → OPAQUE (the inliner folds round(0.7) = 1); `1 − (alpha < 0.3)`
# of an image's alpha → MASK 0.3 with an alpha-only base colour image (white RGB, the image's
# alpha packed into a new PNG); a constant 0 → MASK; an image's Color and Alpha on Base Color
# and Alpha → BLEND with the image re-encoded RGBA (its pixels were edited, so not passed through).
# golden: startup=factory tier=T2 compare=glb:out/alpha.glb
import bpy
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Alpha'].default_value = 0.5
bpy.data.materials['Material'].blend_method = 'OPAQUE'
bpy.data.meshes['Cube'].materials.append(bpy.data.materials.new('Round'))
bpy.data.materials['Round'].node_tree.nodes.new('ShaderNodeMath')
bpy.data.materials['Round'].node_tree.nodes['Math'].operation = 'ROUND'
bpy.data.materials['Round'].node_tree.nodes['Math'].inputs[0].default_value = 0.7
bpy.data.materials['Round'].node_tree.links.new(bpy.data.materials['Round'].node_tree.nodes['Math'].outputs[0], bpy.data.materials['Round'].node_tree.nodes['Principled BSDF'].inputs['Alpha'])
bpy.data.images.new('Mask', 2, 2, alpha=True)
bpy.data.images['Mask'].pixels = [0.1, 0.2, 0.3, 0.0, 0.4, 0.5, 0.6, 0.25, 0.7, 0.8, 0.9, 0.5, 1.0, 0.0, 0.5, 1.0]
bpy.data.meshes['Cube'].materials.append(bpy.data.materials.new('Clip'))
bpy.data.materials['Clip'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Clip'].node_tree.nodes['Image Texture'].image = bpy.data.images['Mask']
bpy.data.materials['Clip'].node_tree.nodes.new('ShaderNodeMath')
bpy.data.materials['Clip'].node_tree.nodes.new('ShaderNodeMath')
bpy.data.materials['Clip'].node_tree.nodes['Math'].operation = 'SUBTRACT'
bpy.data.materials['Clip'].node_tree.nodes['Math'].inputs[0].default_value = 1.0
bpy.data.materials['Clip'].node_tree.nodes['Math.001'].operation = 'LESS_THAN'
bpy.data.materials['Clip'].node_tree.nodes['Math.001'].inputs[1].default_value = 0.3
bpy.data.materials['Clip'].node_tree.links.new(bpy.data.materials['Clip'].node_tree.nodes['Image Texture'].outputs['Alpha'], bpy.data.materials['Clip'].node_tree.nodes['Math.001'].inputs[0])
bpy.data.materials['Clip'].node_tree.links.new(bpy.data.materials['Clip'].node_tree.nodes['Math.001'].outputs[0], bpy.data.materials['Clip'].node_tree.nodes['Math'].inputs[1])
bpy.data.materials['Clip'].node_tree.links.new(bpy.data.materials['Clip'].node_tree.nodes['Math'].outputs[0], bpy.data.materials['Clip'].node_tree.nodes['Principled BSDF'].inputs['Alpha'])
bpy.data.materials['Clip'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.2, 0.6, 0.4, 1.0)
bpy.data.meshes['Cube'].materials.append(bpy.data.materials.new('Zero'))
bpy.data.materials['Zero'].node_tree.nodes['Principled BSDF'].inputs['Alpha'].default_value = 0.0
bpy.data.images.new('Leaf', 2, 2, alpha=True)
bpy.data.images['Leaf'].pixels = [1.0, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.5, 0.0, 0.0, 1.0, 0.25, 1.0, 1.0, 1.0, 0.0]
bpy.data.meshes['Cube'].materials.append(bpy.data.materials.new('Leaf'))
bpy.data.materials['Leaf'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Leaf'].node_tree.nodes['Image Texture'].image = bpy.data.images['Leaf']
bpy.data.materials['Leaf'].node_tree.links.new(bpy.data.materials['Leaf'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Leaf'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.data.materials['Leaf'].node_tree.links.new(bpy.data.materials['Leaf'].node_tree.nodes['Image Texture'].outputs['Alpha'], bpy.data.materials['Leaf'].node_tree.nodes['Principled BSDF'].inputs['Alpha'])
bpy.data.meshes['Cube'].polygons[1].material_index = 1
bpy.data.meshes['Cube'].polygons[2].material_index = 2
bpy.data.meshes['Cube'].polygons[3].material_index = 3
bpy.data.meshes['Cube'].polygons[4].material_index = 4
bpy.ops.export_scene.gltf(filepath='out/alpha.glb')
