# WP 1.23: file operator errors with Blender's messages: saving an unsaved file without a path,
# a missing directory, opening a missing file, a directory, and revert on an unsaved file.
# golden: startup=factory tier=T0
import bpy
bpy.ops.wm.save_mainfile()
bpy.ops.wm.save_as_mainfile()
bpy.ops.wm.save_as_mainfile(filepath='missing/dir/a.frap')
bpy.ops.wm.open_mainfile(filepath='out/nope.frap')
bpy.ops.wm.revert_mainfile()
bpy.ops.wm.revert_mainfile.poll()
