# WP 1.20 (T0), object.move_to_collection / link_to_collection: collection_uid must be set
# ('No collection selected') and name a collection of the scene by session_uid ('Unexpected
# error, collection not found' for -1 and for a collection outside the scene); is_new creates a
# child (empty name → 'Collection.001'); moving unlinks from every other collection of the
# scene, the master included; linking a single object already there is an error, several
# objects are linked silently.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.ops.object.move_to_collection.poll()
bpy.ops.object.move_to_collection()
bpy.ops.object.move_to_collection(collection_uid=-1, is_new=True, new_collection_name='New')
bpy.data.collections.new('Orphan')
bpy.ops.object.move_to_collection(collection_uid=bpy.data.collections['Orphan'].session_uid)
bpy.ops.object.link_to_collection(collection_uid=bpy.data.collections['Collection'].session_uid)
bpy.ops.object.move_to_collection(collection_uid=bpy.data.collections['Collection'].session_uid, is_new=True, new_collection_name='Sub')
bpy.data.collections['Sub'].objects['Cube']
bpy.data.collections['Collection'].children['Sub']
bpy.data.collections['Collection'].objects['Cube']
bpy.ops.object.move_to_collection(collection_uid=bpy.context.scene.collection.session_uid)
bpy.context.scene.collection.objects['Cube']
bpy.data.collections['Sub'].objects['Cube']
bpy.ops.object.move_to_collection(collection_uid=bpy.context.scene.collection.session_uid, is_new=True)
bpy.data.collections['Collection.001'].objects['Cube']
bpy.context.scene.collection.objects['Cube']
bpy.data.objects['Light'].select_set(True)
bpy.ops.object.link_to_collection(collection_uid=bpy.data.collections['Collection'].session_uid)
bpy.data.collections['Collection'].objects['Cube']
bpy.ops.object.link_to_collection(collection_uid=bpy.data.collections['Sub'].session_uid, is_new=True, new_collection_name='L')
bpy.data.collections['Sub'].children['L'].objects['Light']
bpy.ops.object.move_to_collection(collection_uid=bpy.data.collections['L'].session_uid)
bpy.data.collections['Collection'].objects['Light']
bpy.ops.object.select_all(action='DESELECT')
bpy.ops.object.move_to_collection(collection_uid=bpy.data.collections['Collection'].session_uid)
