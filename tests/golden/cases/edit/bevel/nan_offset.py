# WP 2.11: bmo_bevel_exec bevels only when offset > 0, so a NaN offset (or offset_pct) bevels
# nothing; the operator still finishes and deselects everything (offset != 0).
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.bevel(offset=float('nan'), segments=2)
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.bevel(offset_type='PERCENT', offset_pct=float('nan'))
