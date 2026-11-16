# WP 1.10: only BASE_SELECTABLE gates select_set: an object under a layer collection with
# hide_viewport is selected. Deselecting or unhiding an object without a base is a no-op;
# selecting or hiding it raises.
# golden: startup=factory tier=T0 compare=context
import bpy
bpy.context.view_layer.layer_collection.children['Collection'].hide_viewport = True
bpy.data.objects['Cube'].select_set(False)
bpy.data.objects['Light'].select_set(True)
bpy.context.view_layer.layer_collection.children['Collection'].hide_viewport = False
bpy.data.objects.new('Orphan', None)
bpy.data.objects['Orphan'].select_set(False)
bpy.data.objects['Orphan'].hide_set(False)
bpy.data.objects['Orphan'].select_set(True)
bpy.data.objects['Orphan'].hide_set(True)
