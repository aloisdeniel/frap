# WP 1.10: per-view-layer selection, hiding and the active object (§8).
# golden: startup=factory tier=T0 compare=context,objects
import bpy
bpy.data.objects['Camera'].select_set(True)
bpy.data.objects['Cube'].hide_set(True)
bpy.data.objects['Cube'].select_set(True)
bpy.data.objects['Light'].select_set(state=True)
bpy.context.view_layer.objects.active = bpy.data.objects['Light']
bpy.data.objects['Camera'].hide_select = True
bpy.data.objects['Cube'].hide_set(False)
bpy.data.objects['Cube'].select_set(True)
bpy.context.view_layer.objects.active = None
bpy.context.object = bpy.data.objects['Cube']
