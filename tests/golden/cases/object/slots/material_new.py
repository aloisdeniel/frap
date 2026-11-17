# WP 1.20 (T0), material.new from a script: 'Material.001' with the default node graph (as
# bpy.data.materials.new), not assigned to any slot; the poll needs an active mesh that is not
# hidden in the viewport ('Context missing active object', 'Cannot edit hidden object'; hide_set
# does not count; a camera fails with 'failed, context is incorrect').
# golden: startup=factory compare=objects,context,materials tier=T0
import bpy
bpy.ops.material.new()
bpy.data.materials['Material.001'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value
bpy.data.objects['Cube'].hide_set(True)
bpy.ops.material.new()
bpy.data.objects['Cube'].hide_set(False)
bpy.data.objects['Cube'].hide_viewport = True
bpy.ops.material.new.poll()
bpy.ops.material.new()
bpy.data.objects['Cube'].hide_viewport = False
bpy.context.view_layer.objects.active = bpy.data.objects['Camera']
bpy.ops.material.new()
bpy.context.view_layer.objects.active = None
bpy.ops.material.new()
