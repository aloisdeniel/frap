# WP 8.3.6 (T0): object.modifier_convert converts particle systems only (out of scope in frap):
# its poll (edit_modifier_poll) needs an active local object and allows Edit Mode; any other
# modifier, or a name the active object does not have, is CANCELLED and changes nothing.
# golden: startup=factory compare=objects,mesh:Cube tier=T0
import bpy
bpy.ops.object.modifier_convert.poll()
bpy.ops.object.modifier_add(type='SUBSURF')
bpy.ops.object.modifier_convert(modifier='Subdivision')
bpy.ops.object.modifier_convert(modifier='Nope')
bpy.ops.object.modifier_convert()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.object.modifier_convert.poll()
bpy.ops.object.modifier_convert(modifier='Subdivision')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.view_layer.objects.active = None
bpy.ops.object.modifier_convert.poll()
bpy.ops.object.modifier_convert(modifier='Subdivision')
