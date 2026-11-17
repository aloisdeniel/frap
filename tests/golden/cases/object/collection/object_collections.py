# WP 1.20 (T0), the Properties-editor collection operators from a script: collection_add puts
# the active object into a new fake-user collection outside the scene; collection_link links it
# to a local collection by name (an unknown name is an enum error, an existing link is a
# no-op); collection_remove has no collection pointer and is CANCELLED; collection_unlink
# deletes bpy.context.collection, after which the scene collection is active and the poll fails.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.ops.object.collection_add()
bpy.data.collections['Collection.001'].objects['Cube']
bpy.data.collections['Collection.001'].use_fake_user
bpy.ops.object.collection_add()
bpy.data.collections.new('Orphan')
bpy.ops.object.collection_link(collection='Orphan')
bpy.data.collections['Orphan'].objects['Cube']
bpy.ops.object.collection_link(collection='Orphan')
bpy.ops.object.collection_link(collection='Nope')
bpy.ops.object.collection_remove.poll()
bpy.ops.object.collection_remove()
bpy.ops.object.collection_unlink.poll()
bpy.ops.object.collection_unlink()
bpy.data.collections['Collection']
bpy.context.collection.name
bpy.ops.object.collection_unlink.poll()
bpy.ops.object.collection_unlink()
bpy.context.view_layer.objects.active = None
bpy.ops.object.collection_add()
bpy.ops.object.collection_link(collection='Orphan')
