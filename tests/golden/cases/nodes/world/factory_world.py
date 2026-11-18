# WP 3.1: the factory World's node tree ("Shader Nodetree": `World Output` first at (200, 100),
# then `Background` at (-200, 100) with the world colour 0.0509 and Strength 1) and edits of it
# through the node RNA: socket writes by name and index, a second Background and links.new
# replacing the Surface link. Values are copied onto objects (and names onto object names, last)
# so the `objects` comparator sees them.
# golden: startup=factory tier=T0 compare=objects
import bpy
bpy.data.objects['Cube'].location = bpy.data.worlds['World'].color
bpy.data.objects['Cube'].scale = bpy.data.worlds['World'].node_tree.nodes['Background'].inputs['Color'].default_value[0:3]
bpy.data.objects['Cube'].rotation_euler.x = bpy.data.worlds['World'].node_tree.nodes[0].location[0]
bpy.data.objects['Cube'].rotation_euler.y = bpy.data.worlds['World'].node_tree.nodes[1].location[0]
bpy.data.objects['Cube'].rotation_euler.z = bpy.data.worlds['World'].node_tree.nodes['Background'].inputs[1].default_value
bpy.data.worlds['World'].node_tree.nodes['Background'].inputs['Strength'].default_value = 2.5
bpy.data.worlds['World'].node_tree.nodes['Background'].inputs[0].default_value = (0.2, 0.3, 0.4, 1)
bpy.data.worlds['World'].node_tree.nodes.new('ShaderNodeBackground')
bpy.data.worlds['World'].node_tree.nodes['Background.001'].inputs['Color'].default_value = (1, 0.5, 0.25, 1)
bpy.data.worlds['World'].node_tree.links.new(bpy.data.worlds['World'].node_tree.nodes['Background.001'].outputs[0], bpy.data.worlds['World'].node_tree.nodes['World Output'].inputs['Surface'])
bpy.data.objects['Light'].location = bpy.data.worlds['World'].node_tree.links[0].from_node.inputs['Color'].default_value[0:3]
bpy.data.objects['Light'].scale = bpy.data.worlds['World'].node_tree.nodes['Background'].inputs['Color'].default_value[0:3]
bpy.data.objects['Light'].rotation_euler.x = bpy.data.worlds['World'].node_tree.nodes['Background'].inputs['Strength'].default_value
bpy.data.worlds['World'].node_tree.nodes['World Output'].is_active_output
bpy.data.worlds['World'].node_tree.nodes['World Output'].target
bpy.data.worlds['World'].node_tree.nodes['Background'].inputs['Weight'].enabled
bpy.data.worlds['World'].node_tree.nodes.new('ShaderNodeOutputMaterial')
bpy.data.worlds['World'].node_tree.nodes.new('ShaderNodeTexEnvironment')
bpy.data.worlds['World'].node_tree.nodes['Environment Texture'].projection = 'MIRROR_BALL'
bpy.data.worlds['World'].node_tree.links.new(bpy.data.worlds['World'].node_tree.nodes['Environment Texture'].outputs['Color'], bpy.data.worlds['World'].node_tree.nodes['Background'].inputs['Color'])
bpy.data.worlds['World'].node_tree.nodes['Background'].inputs['Color'].is_linked
bpy.data.objects['Camera'].name = bpy.data.worlds['World'].node_tree.name
bpy.data.objects['Light'].name = bpy.data.worlds['World'].node_tree.links[0].from_node.name
