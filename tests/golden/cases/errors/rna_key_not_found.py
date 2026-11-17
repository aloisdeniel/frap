# WP 1.26: a string key that names no item (code RNA_KEY_NOT_FOUND, KeyError) in `bpy.data`
# collections, scene/view-layer/collection object lists, node trees, sockets and material
# slots, also as the target of a write. Keys are case-sensitive.
# golden: startup=factory tier=T0 compare=objects
import bpy
bpy.data.objects['Nope']
bpy.data.objects['cube']
bpy.data.meshes['Nope']
bpy.data.materials['Nope']
bpy.data.scenes['Scene'].objects['Nope']
bpy.data.materials['Material'].node_tree.nodes['Nope']
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Nope']
bpy.data.objects['Cube'].material_slots['Nope']
bpy.data.collections['Collection'].objects['Nope']
bpy.context.view_layer.objects['Nope']
bpy.data.objects['Nope'].location = (1, 2, 3)
bpy.data.objects['Cube'].data = bpy.data.meshes['Nope']
bpy.data.objects['Camera']['foo']
