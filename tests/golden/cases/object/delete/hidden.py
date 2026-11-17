# WP 1.19 (T0): `object.delete` deletes `selected_objects` only: a selected object under a layer
# collection hidden in the viewport is kept (CANCELLED when nothing visible is selected); an
# object hidden with hide_set is deselected and survives too.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.ops.object.select_all(action='SELECT')
bpy.context.view_layer.layer_collection.children['Collection'].hide_viewport = True
bpy.ops.object.delete()
bpy.context.view_layer.layer_collection.children['Collection'].hide_viewport = False
bpy.data.objects['Light'].hide_set(True)
bpy.ops.object.delete()
