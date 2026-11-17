# WP 1.26: context members as path roots in the factory scene (VIEW_3D override, D-10):
# `object`, `active_object`, `view_layer.objects.active`, `selected_objects`,
# `selected_editable_objects`, `scene`, `view_layer`, `collection`, `blend_data`, `mode`,
# `engine`, `tool_settings`, `edit_object` (None in Object Mode). Writes through them land on
# the objects they name.
# golden: startup=factory tier=T0 compare=context,objects
import bpy
bpy.context.object.location.z = 2
bpy.context.active_object.scale = (2, 2, 2)
bpy.context.view_layer.objects.active.rotation_euler.x = 1
bpy.context.selected_objects[0].location.x = 3
bpy.context.selected_editable_objects[0].location.y = 4
bpy.context.scene.objects['Camera'].location.x = 1
bpy.context.view_layer.objects['Light'].location.x = 1
bpy.context.collection.objects['Light'].location.y = 4
bpy.context.scene.collection.children['Collection'].objects['Camera'].rotation_euler.x = 1
bpy.context.blend_data.objects['Light'].rotation_euler.y = 1
bpy.context.edit_object
bpy.context.mode
bpy.context.engine
bpy.context.tool_settings
bpy.context.scene.tool_settings
bpy.data.objects['Light'].name = bpy.context.view_layer.name
bpy.data.objects['Camera'].name = bpy.context.engine
bpy.data.objects['Cube'].name = bpy.context.collection.name
