# M1 stabilization: a standard socket's `default_value` has an unbounded hard range
# (`rna_NodeSocketStandard_float_range`): values outside the declaration range are stored as
# given; colour sockets keep their static range (negative components clamp to 0).
# golden: startup=factory tier=T0 compare=materials
import bpy
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Metallic'].default_value = 2
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = -0.5
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['IOR'].default_value = 0.1
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Alpha'].default_value = 3
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (2, -1, 0.5, 3)
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Normal'].default_value = (5, 5, 5)
