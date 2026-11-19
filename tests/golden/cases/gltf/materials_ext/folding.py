# Integration (WP 3.5 folding + WP 3.6 extensions): the add-on reads the inlined tree, where
# Math nodes with constant inputs are folded: Transmission Weight = 0.5 * 0.8 (transmission
# 0.4), IOR = 1 + 0.2 (ior 1.2, written with transmission), Roughness = 0.1 + 0.2.
# golden: startup=factory tier=T2 compare=glb:out/f.glb
import bpy
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeMath')
bpy.data.materials['Material'].node_tree.nodes['Math'].operation = 'MULTIPLY'
bpy.data.materials['Material'].node_tree.nodes['Math'].inputs[0].default_value = 0.5
bpy.data.materials['Material'].node_tree.nodes['Math'].inputs[1].default_value = 0.8
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Math'].outputs[0], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Transmission Weight'])
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeMath')
bpy.data.materials['Material'].node_tree.nodes['Math.001'].operation = 'ADD'
bpy.data.materials['Material'].node_tree.nodes['Math.001'].inputs[0].default_value = 1.0
bpy.data.materials['Material'].node_tree.nodes['Math.001'].inputs[1].default_value = 0.2
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Math.001'].outputs[0], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['IOR'])
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeMath')
bpy.data.materials['Material'].node_tree.nodes['Math.002'].operation = 'ADD'
bpy.data.materials['Material'].node_tree.nodes['Math.002'].inputs[0].default_value = 0.1
bpy.data.materials['Material'].node_tree.nodes['Math.002'].inputs[1].default_value = 0.2
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Math.002'].outputs[0], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Roughness'])
bpy.ops.export_scene.gltf(filepath='out/f.glb')
