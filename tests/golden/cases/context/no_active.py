# WP 1.26: the context after the active object is cleared and the selection emptied:
# `object`/`active_object` are None, so attribute reads and writes through them raise Python's
# NoneType AttributeError; `selected_objects` is empty, so indexing it raises Python's list
# IndexError. Selecting again restores them (selection without an active object).
# golden: startup=factory tier=T0 compare=context,objects
import bpy
bpy.context.view_layer.objects.active = None
bpy.context.object
bpy.context.active_object
bpy.context.object.location
bpy.context.object.location = (1, 2, 3)
bpy.context.active_object.name
bpy.context.selected_objects[0].name
bpy.data.objects['Cube'].select_set(False)
bpy.context.selected_objects
bpy.context.selected_objects[0]
bpy.data.objects['Light'].select_set(True)
bpy.data.objects['Camera'].select_set(True)
bpy.context.selected_objects[1].location.z = 7
bpy.context.object.location
