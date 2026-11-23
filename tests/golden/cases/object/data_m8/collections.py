# WP 8.3.6 (T0): collection.create names a new collection after `name` ('' -> Collection.001),
# gives it a fake user, keeps it out of the scene and links the selected objects;
# collection.objects_add_active / objects_remove / objects_remove_active take the collections
# holding the active object (Scene Collection first, then bpy.data.collections; 'All
# Collections' when there are two or more; unset = the first one); objects_remove reads the
# selection again per collection (objects that lost their last scene collection have no base,
# so the later collections keep them); objects_remove_all clears every collection; an
# instancing cycle is skipped with a WARNING. Membership is checked by lookups (KeyError when
# absent) and recorded by translations of the selected objects.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.ops.collection.create.poll()
bpy.ops.collection.create()
bpy.data.collections['Collection.001'].use_fake_user
bpy.data.collections['Collection.001'].objects['Cube']
bpy.ops.collection.create(name='Mine')
bpy.ops.collection.create(name='Mine')
bpy.data.collections['Mine.001'].users
bpy.data.objects['Light'].select_set(True)
bpy.ops.collection.objects_add_active()
bpy.ops.collection.objects_add_active(collection='Mine')
bpy.data.collections['Mine'].objects['Light']
bpy.ops.collection.objects_add_active(collection='Nope')
bpy.ops.collection.objects_add_active(collection='All Collections')
bpy.data.collections['Mine.001'].objects['Light']
bpy.data.collections['Collection.001'].objects['Light']
bpy.ops.collection.objects_remove(collection='Mine')
bpy.data.collections['Mine'].objects['Light']
bpy.data.collections['Mine'].objects['Cube']
bpy.context.scene.collection.objects.link(bpy.data.objects['Cube'])
bpy.ops.collection.objects_remove(collection='Scene Collection')
bpy.context.scene.collection.objects['Cube']
bpy.data.collections['Collection'].objects['Light']
bpy.ops.transform.translate(value=(0, 0, 1))
bpy.ops.collection.objects_remove(collection='All Collections')
bpy.data.collections['Collection'].objects['Cube']
bpy.data.collections['Collection.001'].objects['Cube']
bpy.data.collections['Mine.001'].objects['Light']
bpy.context.object
bpy.ops.collection.objects_remove_active()
bpy.ops.collection.objects_remove()
bpy.ops.collection.objects_add_active()
bpy.ops.collection.objects_remove_all()
bpy.data.collections['Collection.001'].objects['Cube']
