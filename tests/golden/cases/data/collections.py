# WP 1.22: `bpy.data.collections.new`, `collection.children.link/unlink` (duplicates, self links,
# cycles and the master collection are refused with "already in collection"),
# `collection.objects.link/unlink`; after `view_layer.update()` the new bases can be selected
# and `selected_objects` shows the base order (child collections before the parent's objects).
# golden: startup=factory tier=T0 compare=objects,context
import bpy
bpy.data.collections.new('A')
bpy.data.collections.new('A')
bpy.data.collections.new('')
bpy.data.collections.new(None)
bpy.data.collections.new(name='Z', x=1)
bpy.context.scene.collection.children.link(bpy.data.collections['A'])
bpy.context.scene.collection.children.link(bpy.data.collections['A'])
bpy.data.collections['A'].children.link(bpy.data.collections['A'])
bpy.data.collections['A'].children.link(bpy.data.collections['A.001'])
bpy.data.collections['A.001'].children.link(bpy.data.collections['A'])
bpy.data.collections['Collection'].children.link(bpy.context.scene.collection)
bpy.data.collections['A.001'].children.link(bpy.data.collections['Collection'])
bpy.context.scene.collection.children.link(None)
bpy.data.objects.new('E1', None)
bpy.data.objects.new('E2', None)
bpy.data.collections['A.001'].objects.link(bpy.data.objects['E1'])
bpy.data.collections['A'].objects.link(bpy.data.objects['E2'])
bpy.data.collections['A'].objects.link(bpy.data.objects['Cube'])
bpy.data.collections['Collection'].objects.unlink(bpy.data.objects['Camera'])
bpy.data.collections['Collection'].objects.unlink(bpy.data.objects['Camera'])
bpy.data.collections['A'].objects.unlink(bpy.data.objects['E1'])
bpy.context.scene.collection.children.unlink(bpy.data.collections['Collection'])
bpy.context.scene.collection.children.unlink(bpy.data.collections['Collection'])
bpy.data.collections['A'].children.unlink(bpy.data.collections['Collection'])
bpy.context.view_layer.update()
bpy.data.objects['E1'].select_set(True)
bpy.data.objects['E2'].select_set(True)
bpy.data.objects['Light'].select_set(True)
bpy.data.objects['Camera'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['E2']
