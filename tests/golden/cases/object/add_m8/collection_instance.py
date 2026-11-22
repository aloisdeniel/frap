# WP 8.3.4: object.collection_instance_add finds the collection by session_uid, else by name
# (when passed), else through the collection enum (the first collection when unset; an unknown
# name falls back to it); the empty is named after the collection, placed (location, rotation,
# scale, align) and instances it (read back through instance_collection); an active collection
# that is (or is inside) the instanced collection moves to its closest visible ancestor, where
# the instance goes; an unknown enum item is a TypeError; Edit Mode fails the poll.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.data.collections.new('Src')
bpy.context.scene.collection.children.link(bpy.data.collections['Src'])
bpy.data.collections.new('Deep')
bpy.data.collections['Src'].children.link(bpy.data.collections['Deep'])
bpy.data.collections['Src'].objects.link(bpy.data.objects.new('S1', None))
bpy.ops.object.collection_instance_add(collection='Src', location=(1, 0, 0))
bpy.data.objects['Src'].instance_collection.name
bpy.ops.object.collection_instance_add(name='Src', location=(5, 0, 0), rotation=(0, 0, 1.5707963), scale=(2, 2, 2))
bpy.ops.object.collection_instance_add(name='Nope', location=(0, 1, 0))
bpy.data.objects['Collection'].instance_collection.name
bpy.ops.object.collection_instance_add(session_uid=bpy.data.collections['Deep'].session_uid, align='VIEW')
bpy.ops.object.collection_instance_add()
bpy.ops.object.collection_instance_add(collection='Deep', align='CURSOR')
bpy.context.view_layer.active_layer_collection = bpy.context.view_layer.layer_collection.children['Src'].children['Deep']
bpy.ops.object.collection_instance_add(name='Src', location=(0, 0, 3))
bpy.ops.object.empty_add(location=(0, 0, 9))
bpy.ops.object.collection_instance_add(collection='Missing')
bpy.data.objects['Empty'].instance_collection.name
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.object.select_all(action='DESELECT')
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.object.collection_instance_add(collection='Src')
