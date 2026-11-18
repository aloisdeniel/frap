# WP 2.10: separate links each new object like object_add_sync_base_collection: the edit
# object Cube is deselected but visible in the viewport, so Cube.001 goes to every
# collection holding Cube (Collection and Sub), not only the active collection.
# golden: startup=factory compare=objects,context tier=T1
import bpy
bpy.data.collections.new('Sub')
bpy.context.scene.collection.children.link(bpy.data.collections['Sub'])
bpy.data.collections['Sub'].objects.link(bpy.data.objects['Cube'])
bpy.ops.object.mode_set(mode='EDIT')
bpy.data.objects['Cube'].select_set(False)
bpy.ops.frap.select_elements(type='FACE', indices=[5])
bpy.ops.mesh.separate(type='SELECTED')
bpy.data.objects['Cube.001'].users_collection[0].name
bpy.data.objects['Cube.001'].users_collection[1].name
bpy.data.objects['Cube.001'].users_collection[2].name
bpy.data.collections['Sub'].objects[1].name
bpy.data.collections['Collection'].objects[3].name
bpy.context.selected_objects[0].name
bpy.context.selected_objects[1].name
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.separate(type='SELECTED')
bpy.data.objects['Cube.002'].users_collection[1].name
bpy.context.view_layer.objects.active.name
