# WP 3.9: foliage cards. A leaf atlas with distinct RGBA pixels drives Base Color and, through
# `1 - (alpha < 0.5)`, the Alpha (alphaMode MASK, alphaCutoff 0.5); a frosted pane has a
# constant Alpha of 0.35 (BLEND) and backface culling (single-sided); a hedge card uses the
# atlas's Color and Alpha directly (BLEND, RGBA PNG). Round trip (§17.2): the importer rebuilds
# the alpha chains and the re-export matches.
# golden: startup=factory tier=T2 compare=glb:out/foliage.glb,glb:out/foliage_rt.glb glb_structure
import bpy
bpy.data.objects.remove(bpy.data.objects['Cube'])
bpy.data.images.new('Leaves', 2, 2, alpha=True)
bpy.data.images['Leaves'].pixels = [0.1, 0.6, 0.1, 1.0, 0.2, 0.7, 0.2, 0.2, 0.3, 0.5, 0.05, 0.8, 0.0, 0.0, 0.0, 0.0]
bpy.ops.mesh.primitive_plane_add(size=1, location=(0, 0, 1), rotation=(1.5707963, 0, 0))
bpy.data.objects['Plane'].name = 'Leaf'
bpy.data.materials.new('LeafCut')
bpy.data.objects['Leaf'].data.materials.append(bpy.data.materials['LeafCut'])
bpy.data.materials['LeafCut'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['LeafCut'].node_tree.nodes['Image Texture'].image = bpy.data.images['Leaves']
bpy.data.materials['LeafCut'].node_tree.links.new(bpy.data.materials['LeafCut'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['LeafCut'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.data.materials['LeafCut'].node_tree.nodes.new('ShaderNodeMath')
bpy.data.materials['LeafCut'].node_tree.nodes.new('ShaderNodeMath')
bpy.data.materials['LeafCut'].node_tree.nodes['Math'].operation = 'SUBTRACT'
bpy.data.materials['LeafCut'].node_tree.nodes['Math'].inputs[0].default_value = 1.0
bpy.data.materials['LeafCut'].node_tree.nodes['Math.001'].operation = 'LESS_THAN'
bpy.data.materials['LeafCut'].node_tree.nodes['Math.001'].inputs[1].default_value = 0.5
bpy.data.materials['LeafCut'].node_tree.links.new(bpy.data.materials['LeafCut'].node_tree.nodes['Image Texture'].outputs['Alpha'], bpy.data.materials['LeafCut'].node_tree.nodes['Math.001'].inputs[0])
bpy.data.materials['LeafCut'].node_tree.links.new(bpy.data.materials['LeafCut'].node_tree.nodes['Math.001'].outputs[0], bpy.data.materials['LeafCut'].node_tree.nodes['Math'].inputs[1])
bpy.data.materials['LeafCut'].node_tree.links.new(bpy.data.materials['LeafCut'].node_tree.nodes['Math'].outputs[0], bpy.data.materials['LeafCut'].node_tree.nodes['Principled BSDF'].inputs['Alpha'])
bpy.ops.mesh.primitive_plane_add(size=2, location=(3, 0, 1), rotation=(1.5707963, 0, 0))
bpy.data.objects['Plane'].name = 'Pane'
bpy.data.materials.new('Frosted')
bpy.data.objects['Pane'].data.materials.append(bpy.data.materials['Frosted'])
bpy.data.materials['Frosted'].node_tree.nodes['Principled BSDF'].inputs['Alpha'].default_value = 0.35
bpy.data.materials['Frosted'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.8, 0.9, 1.0, 1.0)
bpy.data.materials['Frosted'].use_backface_culling = True
bpy.ops.mesh.primitive_plane_add(size=1, location=(-3, 0, 1))
bpy.data.objects['Plane'].name = 'Hedge'
bpy.data.materials.new('HedgeCard')
bpy.data.objects['Hedge'].data.materials.append(bpy.data.materials['HedgeCard'])
bpy.data.materials['HedgeCard'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['HedgeCard'].node_tree.nodes['Image Texture'].image = bpy.data.images['Leaves']
bpy.data.materials['HedgeCard'].node_tree.links.new(bpy.data.materials['HedgeCard'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['HedgeCard'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.data.materials['HedgeCard'].node_tree.links.new(bpy.data.materials['HedgeCard'].node_tree.nodes['Image Texture'].outputs['Alpha'], bpy.data.materials['HedgeCard'].node_tree.nodes['Principled BSDF'].inputs['Alpha'])
bpy.ops.export_scene.gltf(filepath='out/foliage.glb')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/foliage.glb')
bpy.ops.export_scene.gltf(filepath='out/foliage_rt.glb')
