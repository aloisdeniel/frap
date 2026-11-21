# WP 7.6: an animated Node.width is clamped to the node type's width range
# (rna_Node_width_range: bl_width_min..bl_width_max, 100..700 for the Principled BSDF and the
# Material Output), not to the static (unbounded) range. Keys 240@1, 1000@10, 20@20 on the
# Principled BSDF and 50@1, 900@10 on the output read 700 / 100 and 100 / 700; the evaluated
# widths are copied into probe empties.
# golden: startup=factory tier=T1 tol=1e-4 compare=objects
import bpy
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].keyframe_insert('width', frame=1)
bpy.data.materials['Material'].node_tree.nodes['Material Output'].width = 140.0
bpy.data.materials['Material'].node_tree.nodes['Material Output'].keyframe_insert('width', frame=1)
bpy.data.materials['Material'].node_tree.animation_data.action.layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points.insert(10, 1000.0)
bpy.data.materials['Material'].node_tree.animation_data.action.layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points.insert(20, 20.0)
bpy.data.materials['Material'].node_tree.animation_data.action.layers[0].strips[0].channelbags[0].fcurves[1].keyframe_points.insert(1, 50.0)
bpy.data.materials['Material'].node_tree.animation_data.action.layers[0].strips[0].channelbags[0].fcurves[1].keyframe_points.insert(10, 900.0)
bpy.context.scene.frame_set(1)
bpy.context.scene.collection.objects.link(bpy.data.objects.new('w1', None))
bpy.data.objects['w1'].location = (bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].width, bpy.data.materials['Material'].node_tree.nodes['Material Output'].width, 0.0)
bpy.context.scene.frame_set(10)
bpy.context.scene.collection.objects.link(bpy.data.objects.new('w10', None))
bpy.data.objects['w10'].location = (bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].width, bpy.data.materials['Material'].node_tree.nodes['Material Output'].width, 0.0)
bpy.context.scene.frame_set(20)
bpy.context.scene.collection.objects.link(bpy.data.objects.new('w20', None))
bpy.data.objects['w20'].location = (bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].width, bpy.data.materials['Material'].node_tree.nodes['Material Output'].width, 0.0)
bpy.context.scene.frame_set(5)
bpy.context.scene.collection.objects.link(bpy.data.objects.new('w5', None))
bpy.data.objects['w5'].location = (bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].width, bpy.data.materials['Material'].node_tree.nodes['Material Output'].width, 0.0)
