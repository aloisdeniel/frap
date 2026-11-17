# WP 1.22: `node_tree.nodes.new/remove`, `node_tree.links.new/remove` on material trees: unique
# node names, reversed socket order, input replacement with `verify_limits`, sockets of another
# tree (links.new returns None), nodes/links of another tree (errors), unknown node types.
# golden: startup=factory tier=T0 compare=materials
import bpy
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeMath')
bpy.data.materials['Material'].node_tree.nodes.new(type='ShaderNodeMath').name
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeRGB')
bpy.data.materials['Material'].node_tree.nodes.new('Foo')
bpy.data.materials['Material'].node_tree.nodes.new('GeometryNodeMeshCube')
bpy.data.materials['Material'].node_tree.nodes.new()
bpy.data.materials['Material'].node_tree.nodes.new(1)
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Math'].outputs[0], bpy.data.materials['Material'].node_tree.nodes['Math.001'].inputs[0])
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Math.001'].inputs[1], bpy.data.materials['Material'].node_tree.nodes['Math'].outputs[0]).from_node.name
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Math.001'].inputs[1], bpy.data.materials['Material'].node_tree.nodes['Math'].inputs[0])
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Color'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Math.001'].outputs[0], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Roughness'])
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Math'].outputs[0], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Roughness'])
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Math'].outputs[0], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Metallic'], verify_limits=False)
bpy.data.materials['Material'].node_tree.links.new(bpy.data.worlds['World'].node_tree.nodes['Background'].outputs[0], bpy.data.materials['Material'].node_tree.nodes['Math'].inputs[0])
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Math'].outputs[0])
bpy.data.materials['Material'].node_tree.links.remove(bpy.data.worlds['World'].node_tree.links[0])
bpy.data.materials['Material'].node_tree.links.remove(None)
bpy.data.materials['Material'].node_tree.nodes.remove(bpy.data.worlds['World'].node_tree.nodes['World Output'])
bpy.data.materials['Material'].node_tree.nodes.remove(None)
bpy.data.materials['Material'].node_tree.links.remove(bpy.data.materials['Material'].node_tree.links[1])
bpy.data.materials.new('Second')
bpy.data.materials['Second'].node_tree.nodes.new('ShaderNodeMath')
bpy.data.materials['Second'].node_tree.links.new(bpy.data.materials['Second'].node_tree.nodes['Math'].outputs[0], bpy.data.materials['Second'].node_tree.nodes['Principled BSDF'].inputs['Alpha'])
bpy.data.materials['Second'].node_tree.nodes.remove(bpy.data.materials['Second'].node_tree.nodes['Math'])
bpy.data.materials['Second'].node_tree.nodes.remove(bpy.data.materials['Second'].node_tree.nodes['Material Output'])
bpy.data.materials['Second'].node_tree.nodes.new('ShaderNodeOutputMaterial').name
