# WP 2.5: mesh.inset with the default UV interpolation on three faces around a cube corner,
# with depth: the inset ring's corner UVs are interpolated and merged across the region's
# inner edges (bm_loop_customdata_merge).
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[0, 4, 5])
bpy.ops.mesh.inset(thickness=0.15, depth=0.05)
