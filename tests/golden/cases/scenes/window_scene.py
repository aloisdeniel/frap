# WP 3.10: `bpy.context.window.scene = …` switches `context.scene`, `context.view_layer`,
# `context.collection` and the active/selected objects; operators then work in that scene
# (a primitive lands in its scene collection). `None` is refused. A scene whose view layer has
# another name shows its first view layer. Switching while an object is in Edit Mode leaves
# it in Edit Mode: `context.mode` follows the new scene's active object, and coming back
# finds Edit Mode again.
# golden: startup=factory tier=T0 compare=objects,context
import bpy
bpy.data.scenes.new('S2')
bpy.context.window.scene = None
bpy.context.window.scene = bpy.data.scenes['S2']
bpy.context.scene.name
bpy.context.view_layer.name
bpy.context.view_layer.objects.active
bpy.ops.mesh.primitive_cylinder_add(vertices=6, location=(0, 0, 3))
bpy.context.view_layer.objects.active.name
bpy.context.window.scene = bpy.data.scenes['S2']
bpy.data.scenes['S2'].view_layers['ViewLayer'].name = 'Other'
bpy.context.view_layer.name
bpy.context.window.view_layer = bpy.data.scenes['Scene'].view_layers['ViewLayer']
bpy.context.window.scene = bpy.data.scenes['Scene']
bpy.context.view_layer.name
bpy.context.window.scene = bpy.data.scenes['S2']
bpy.context.view_layer.name
bpy.context.scene.collection.objects.link(bpy.data.objects.new(bpy.context.view_layer.name, None))
bpy.context.window.scene = bpy.data.scenes['Scene']
bpy.context.view_layer.name
bpy.context.collection.name
bpy.ops.object.mode_set(mode='EDIT')
bpy.context.window.scene = bpy.data.scenes['S2']
bpy.context.mode
bpy.ops.object.mode_set(mode='EDIT')
bpy.context.window.scene = bpy.data.scenes['Scene']
bpy.context.mode
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.scene.collection.objects.link(bpy.data.objects.new(bpy.context.mode, None))
bpy.context.scene.collection.objects.link(bpy.data.objects.new(bpy.context.scene.name, None))
bpy.context.scene.collection.objects.link(bpy.data.objects.new(bpy.context.collection.name, None))
