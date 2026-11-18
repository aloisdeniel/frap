# WP 3.10 (M34W0-4): `object.hide_view_set` hides (and deselects) the visible selected objects
# of the view layer, `unselected=True` the others; nothing to hide is CANCELLED.
# `object.hide_view_clear` shows every hidden object and selects it unless `select=False`;
# nothing hidden is CANCELLED. `Object.hide_viewport` is untouched; hiding is per view layer.
# An object that cannot be selected (`hide_select`) is shown but not selected.
# golden: startup=factory tier=T0 compare=objects,context
import bpy
bpy.ops.object.hide_view_set()
bpy.data.objects['Cube'].hide_get()
bpy.ops.object.hide_view_set()
bpy.data.objects['Light'].select_set(True)
bpy.ops.object.hide_view_set(unselected=True)
bpy.context.scene.collection.objects.link(bpy.data.objects.new('H1', None))
bpy.data.objects['H1'].location = (bpy.data.objects['Cube'].hide_get(), bpy.data.objects['Light'].hide_get(), bpy.data.objects['Camera'].hide_get())
bpy.data.objects['H1'].scale = (bpy.data.objects['Cube'].select_get(), bpy.data.objects['Light'].select_get(), bpy.data.objects['Camera'].hide_viewport)
bpy.ops.object.hide_view_clear(select=False)
bpy.context.scene.collection.objects.link(bpy.data.objects.new('H2', None))
bpy.data.objects['H2'].location = (bpy.data.objects['Cube'].hide_get(), bpy.data.objects['Light'].hide_get(), bpy.data.objects['Camera'].hide_get())
bpy.data.objects['H2'].scale = (bpy.data.objects['Cube'].select_get(), bpy.data.objects['Light'].select_get(), bpy.data.objects['Camera'].select_get())
bpy.ops.object.hide_view_clear()
bpy.data.objects['Camera'].hide_viewport = True
bpy.ops.object.select_all(action='SELECT')
bpy.ops.object.hide_view_set()
bpy.data.objects['Camera'].hide_viewport = False
bpy.ops.object.hide_view_clear()
bpy.ops.scene.new(type='LINK_COPY')
bpy.ops.object.hide_view_set()
bpy.context.window.scene = bpy.data.scenes['Scene']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.object.hide_view_set()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.scene.collection.objects.link(bpy.data.objects.new('H3', None))
bpy.data.objects['H3'].location = (bpy.data.objects['Cube'].hide_get(), bpy.data.objects['Light'].hide_get(), bpy.data.objects['Camera'].hide_get())
bpy.ops.object.select_all(action='SELECT')
bpy.data.objects['Light'].hide_select = True
bpy.ops.object.select_all(action='DESELECT')
bpy.ops.object.hide_view_set(unselected=True)
bpy.ops.object.hide_view_clear()
