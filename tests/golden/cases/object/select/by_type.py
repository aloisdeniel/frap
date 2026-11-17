# WP 1.20 (T0), object.select_by_type: extend=False deselects the visible objects first;
# extend=True adds; no match with extend=False leaves nothing selected; an object hidden with
# hide_set or unselectable (hide_select) is not selected; the active object never changes.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.ops.object.select_by_type(type='LIGHT')
bpy.ops.object.select_by_type(type='CAMERA', extend=True)
bpy.ops.object.select_by_type(type='EMPTY', extend=True)
bpy.ops.object.select_by_type(type='EMPTY')
bpy.data.objects['Light'].hide_set(True)
bpy.ops.object.select_by_type(type='LIGHT')
bpy.data.objects['Light'].hide_set(False)
bpy.data.objects['Light'].hide_select = True
bpy.ops.object.select_by_type(type='LIGHT')
bpy.data.objects['Light'].hide_select = False
bpy.ops.object.empty_add()
bpy.ops.object.select_by_type(type='MESH', extend=True)
bpy.context.view_layer.objects.active = None
bpy.ops.object.select_by_type.poll()
bpy.ops.object.select_by_type(type='LIGHT', extend=True)
bpy.ops.object.select_by_type(type='MESHH')
