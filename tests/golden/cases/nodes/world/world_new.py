# WP 3.1: `bpy.ops.world.new()` from a script adds `World.001` (colour 0.05 grey, not the factory
# 0.0509) with the default graph: `Background` (colour copied from the world, Strength 1) at
# (-200, 100) linked to `World Output` at (200, 100) in an embedded tree "World Nodetree"; the
# scene keeps its world, also when it has none. `bpy.data.worlds.new()` gives the same world.
# Values are copied onto objects (location, scale, rotation) and names onto object names (last)
# so the `objects` comparator sees them.
# golden: startup=factory tier=T0 compare=objects
import bpy
bpy.ops.world.new.poll()
bpy.ops.world.new()
bpy.data.worlds['World.001'].node_tree.nodes['Background'].inputs['Strength'].default_value
bpy.data.objects['Cube'].location = bpy.data.worlds['World.001'].color
bpy.data.objects['Cube'].scale = bpy.data.worlds['World.001'].node_tree.nodes['Background'].inputs['Color'].default_value[0:3]
bpy.data.objects['Cube'].rotation_euler.x = bpy.data.worlds['World.001'].node_tree.nodes[0].location[0]
bpy.data.objects['Cube'].rotation_euler.y = bpy.data.worlds['World.001'].node_tree.nodes[1].location[0]
bpy.data.objects['Cube'].rotation_euler.z = bpy.data.worlds['World.001'].node_tree.nodes['Background'].inputs['Strength'].default_value
bpy.data.worlds['World.001'].node_tree.links[0].from_node.name
bpy.data.worlds['World.001'].node_tree.links[0].to_socket.name
bpy.data.worlds.new('Sky')
bpy.data.objects['Light'].location = bpy.data.worlds['Sky'].color
bpy.data.objects['Light'].scale = bpy.data.worlds['Sky'].node_tree.nodes['Background'].inputs['Color'].default_value[0:3]
bpy.data.objects['Light'].rotation_euler.x = bpy.data.worlds['Sky'].node_tree.nodes[1].location[0]
bpy.data.objects['Light'].name = bpy.context.scene.world.name
bpy.context.scene.world = None
bpy.ops.world.new()
bpy.data.worlds['World.002'].node_tree.nodes['World Output'].location[1]
bpy.data.objects['Camera'].location.x = bpy.data.worlds['World.001'].node_tree.nodes['World Output'].location[1]
bpy.data.objects['Camera'].name = bpy.data.worlds['World.002'].node_tree.name
