# WP 1.23: save_mainfile reuses bpy.data.filepath; revert_mainfile reloads it; a copy keeps
# the current file path.
# Paths are in the work directory: Blender's save does not create directories.
# golden: startup=factory tier=T1 compare=objects,context tol=1e-6
import bpy
bpy.ops.wm.save_mainfile()
bpy.ops.wm.revert_mainfile()
bpy.ops.wm.save_as_mainfile(filepath='first.frap')
bpy.data.objects['Cube'].location = (1, 1, 1)
bpy.ops.wm.save_mainfile()
bpy.ops.wm.save_as_mainfile(filepath='copy.frap', copy=True)
bpy.data.objects['Cube'].location = (2, 2, 2)
bpy.ops.wm.revert_mainfile()
bpy.ops.wm.open_mainfile(filepath='copy.frap')
