# WP 1.10: the `(name, library)` tuple key of `bpy.data` collections (§5.2, T0). `None` names a
# local ID; a string names a library by filepath; a string that is no library's filepath and a
# missing name are KeyErrors with different messages.
# golden: startup=factory tier=T0 compare=objects
import bpy
bpy.data.objects['Cube', None]
bpy.data.objects['Nope', None]
bpy.data.objects['Cube', '/nonexistent/library.blend']
bpy.data.objects['Cube', None].location = (1, 2, 3)
