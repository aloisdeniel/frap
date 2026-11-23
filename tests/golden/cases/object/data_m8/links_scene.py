# WP 8.3.6 (T0): object.make_links_scene links the selected objects to another scene's master
# collection (scene items by name, in bpy.data.scenes order; unset = the first scene, here
# 'Other', sorted before 'Scene'); the context scene or an unknown name fails.
# object.unlink_data always fails from a script (it needs the template_ID button).
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.ops.object.make_links_scene.poll()
bpy.ops.object.make_links_scene()
bpy.ops.object.make_links_scene(scene='Scene')
bpy.data.scenes.new('Other')
bpy.data.objects['Light'].select_set(True)
bpy.ops.object.make_links_scene(scene='Nope')
bpy.ops.object.make_links_scene()
bpy.data.scenes['Other'].collection.objects['Cube']
bpy.data.scenes['Other'].collection.objects['Light']
bpy.data.scenes['Other'].collection.objects['Camera']
bpy.ops.object.make_links_scene(scene='Other')
bpy.data.scenes['Other'].objects['Light']
bpy.data.objects['Cube'].users_scene
bpy.ops.object.unlink_data.poll()
bpy.ops.object.unlink_data()
