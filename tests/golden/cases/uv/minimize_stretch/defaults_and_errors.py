# WP 8.1: headless uv.minimize_stretch runs exactly `iterations` passes, so the default
# iterations=0 only flushes (the UVs stay); nothing selected: CANCELLED; a partial selection
# relaxes only the selected faces' islands; outside Edit Mode: poll error.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T2 tol=1e-5
import bpy
bpy.ops.uv.minimize_stretch(iterations=1)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.subdivide(number_cuts=2)
bpy.ops.uv.minimize_stretch()
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.uv.minimize_stretch(iterations=3)
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[0, 1, 2, 3, 4, 5, 6, 7, 8])
bpy.ops.uv.minimize_stretch(iterations=4)
bpy.ops.object.mode_set(mode='OBJECT')
