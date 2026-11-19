# WP 3.8: uv.unwrap(method='CONFORMAL') of the default cube cut by the standard 7 seams: a
# cross of 0.25 squares (LSCM, then the pack with the default margin).
# golden: startup=factory compare=objects,context,mesh:Cube tier=T2 tol=1e-4
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.frap.select_elements(type='EDGE', indices=[0, 2, 3, 5, 6, 9, 11])
bpy.ops.mesh.mark_seam()
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.unwrap(method='CONFORMAL')
bpy.ops.object.mode_set(mode='OBJECT')
