# WP 8.3.5 (T0), object.instance_offset_from_cursor / _from_object / _to_cursor on the active
# collection (a new collection, then the scene collection), read back through the 3D cursor;
# instance_offset_from_object needs an active object.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.data.objects['Cube'].location = (1, 2, 3)
bpy.data.objects['Cube'].parent = bpy.data.objects['Light']
bpy.context.scene.cursor.location = (0.5, -1, 2)
bpy.ops.object.instance_offset_from_cursor()
bpy.context.scene.cursor.location = (0, 0, 0)
bpy.ops.object.instance_offset_to_cursor()
bpy.ops.object.instance_offset_from_object()
bpy.context.scene.cursor.location = (9, 9, 9)
bpy.ops.object.instance_offset_to_cursor()
bpy.context.scene.collection.instance_offset = (3, 2, 1)
bpy.context.view_layer.active_layer_collection = bpy.context.view_layer.layer_collection
bpy.ops.object.instance_offset_to_cursor()
bpy.context.scene.cursor.location = (-1, -2, -3)
bpy.ops.object.instance_offset_from_cursor()
bpy.context.scene.cursor.location = (0, 0, 0)
bpy.ops.object.instance_offset_to_cursor()
bpy.context.view_layer.objects.active = None
bpy.ops.object.instance_offset_from_object()
bpy.data.collections['Collection'].instance_offset = (4, 5, 6)
bpy.context.view_layer.active_layer_collection = bpy.context.view_layer.layer_collection.children['Collection']
bpy.ops.object.instance_offset_to_cursor()
