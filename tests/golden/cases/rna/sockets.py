# WP 1.26: material node sockets addressed by name and by index (get and set), colour sockets
# taking four floats, vector sockets three, one component of a colour socket, and the
# material's own viewport colour; the node graph is compared.
# golden: startup=factory tier=T0 compare=materials,objects
import bpy
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (1, 0, 0, 1)
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs[1].default_value = 0.5
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.25
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Normal'].default_value = (1, 0, 0)
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Emission Color'].default_value[1] = 0.5
bpy.data.materials['Material'].diffuse_color = (0.6, 0.6, 0.7, 1.0)
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs[1].default_value
bpy.data.objects['Cube'].location = bpy.data.materials['Material'].diffuse_color[0:3]
bpy.data.objects['Cube'].scale.x = bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Metallic'].default_value
