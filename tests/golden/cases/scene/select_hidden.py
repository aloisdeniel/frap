# WP 1.10: only BASE_SELECTABLE gates select_set: an object under a layer collection with
# hide_viewport is selected. Deselecting or unhiding an object without a base is a no-op;
# selecting or hiding it raises. (The base-less object comes from an excluded collection, not
# from `bpy.data.objects.new`, which lands with WP 1.22 in W3.)
# golden: startup=factory tier=T0 compare=context
import bpy
bpy.context.view_layer.layer_collection.children['Collection'].hide_viewport = True
bpy.data.objects['Cube'].select_set(False)
bpy.data.objects['Light'].select_set(True)
bpy.context.view_layer.layer_collection.children['Collection'].hide_viewport = False
bpy.context.view_layer.layer_collection.children['Collection'].exclude = True
bpy.data.objects['Cube'].select_set(False)
bpy.data.objects['Cube'].hide_set(False)
bpy.data.objects['Cube'].select_set(True)
bpy.data.objects['Cube'].hide_set(True)
