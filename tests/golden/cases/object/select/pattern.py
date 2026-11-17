# WP 1.20 (T0), object.select_pattern (Python's fnmatch on object names): extend=True by default;
# case-insensitive unless case_sensitive; '?', '[seq]', '[!seq]'; no match changes nothing with
# extend, deselects everything without; hidden objects are skipped; works without an active
# object.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.ops.object.select_pattern(pattern='C*')
bpy.ops.object.select_pattern(pattern='l*', case_sensitive=True)
bpy.ops.object.select_pattern(pattern='c*', extend=False)
bpy.ops.object.select_pattern(pattern='L?ght', extend=False)
bpy.ops.object.select_pattern(pattern='[CL]*', extend=False)
bpy.ops.object.select_pattern(pattern='[!C]*', extend=False)
bpy.ops.object.select_pattern(pattern='nomatch')
bpy.ops.object.select_pattern(pattern='nomatch', extend=False)
bpy.context.view_layer.objects.active = None
bpy.ops.object.select_pattern.poll()
bpy.ops.object.select_pattern(pattern='Cube')
bpy.data.objects['Light'].hide_set(True)
bpy.ops.object.select_pattern(pattern='*', extend=False)
bpy.ops.object.select_pattern(pattern='*a*', extend=False, case_sensitive=True)
