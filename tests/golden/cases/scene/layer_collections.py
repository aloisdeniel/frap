# WP 1.10: layer-collection and collection visibility: a hidden layer collection empties
# `selected_objects`; exclusion removes the bases and the active object.
# golden: startup=factory tier=T0 compare=context
import bpy
bpy.data.objects['Light'].select_set(True)
bpy.context.view_layer.layer_collection.children['Collection'].hide_viewport = True
bpy.context.view_layer.layer_collection.children['Collection'].hide_viewport = False
bpy.context.view_layer.layer_collection.children['Collection'].exclude = True
bpy.context.view_layer.layer_collection.children['Collection'].name = 'X'
