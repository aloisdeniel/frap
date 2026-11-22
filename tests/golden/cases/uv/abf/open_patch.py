# WP 8.1: ABF++ on an open, curved patch (a grid with two raised vertex groups): one boundary,
# no seams, many interior vertices whose 3D angle sums differ from 2 pi.
# golden: startup=empty compare=objects,context,mesh:Grid tier=T2 tol=1e-5
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=8, y_subdivisions=8, size=2)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.select_mode(type='VERT')
bpy.ops.frap.select_elements(type='VERT', indices=[20, 21, 29, 30])
bpy.ops.transform.translate(value=(0, 0, 0.6))
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[42, 43])
bpy.ops.transform.translate(value=(0.1, 0, -0.4))
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.unwrap(method='ANGLE_BASED', fill_holes=False)
bpy.ops.object.mode_set(mode='OBJECT')
