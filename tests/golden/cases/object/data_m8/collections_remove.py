# WP 8.3.6 (T0): collection.objects_remove_active with nothing selected raises 'Active object
# contains no collections' (an ERROR report of a finished operator); objects_add_active skips an
# object whose instanced collection would contain itself (cycle WARNING, nothing linked:
# CANCELLED) and links the others; objects_remove_all takes every selected object out of every
# collection (also the ones outside the scene), leaving no base and no active object.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.data.collections.new('Parts')
bpy.data.collections['Parts'].objects.link(bpy.data.objects['Cube'])
bpy.ops.object.empty_add(location=(2, 0, 0))
bpy.context.object.instance_type = 'COLLECTION'
bpy.context.object.instance_collection = bpy.data.collections['Parts']
bpy.data.objects['Cube'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.collection.objects_add_active(collection='Parts')
bpy.data.collections['Parts'].objects['Empty']
bpy.data.objects['Light'].select_set(True)
bpy.ops.collection.objects_add_active(collection='Parts')
bpy.data.collections['Parts'].objects['Light']
bpy.ops.object.select_all(action='DESELECT')
bpy.ops.collection.objects_remove_active(collection='Parts')
bpy.data.objects['Light'].select_set(True)
bpy.ops.collection.objects_remove_active(collection='Parts')
bpy.data.collections['Parts'].objects['Light']
bpy.data.collections['Collection'].objects['Light']
bpy.data.objects['Cube'].select_set(True)
bpy.data.objects['Camera'].select_set(True)
bpy.ops.collection.objects_remove_all()
bpy.data.collections['Parts'].objects['Cube']
bpy.data.collections['Collection'].objects['Cube']
bpy.data.collections['Collection'].objects['Camera']
bpy.data.objects['Cube'].users_collection
bpy.context.object
bpy.context.view_layer.objects.active = bpy.data.objects['Empty']
bpy.ops.collection.objects_remove_all()
bpy.data.objects['Empty'].users_collection
