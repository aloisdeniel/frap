# WP 2.11: every cube edge with one segment (verified counts V24 E48 F26): POLY corners,
# every face (rebuilt ones too) selected in vertex mode.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.bevel(offset=0.2)
