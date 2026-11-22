# WP 8.1: uv.minimize_stretch(blend=0.25, iterations=20) on a curved grid: the result is
# blended with the original UVs (25 % original); user pins are ignored (only the boundary
# stays), and the pin flags themselves are kept.
# golden: startup=empty compare=objects,context,mesh:Grid tier=T2 tol=1e-5
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=8, y_subdivisions=8, size=2)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[20, 21, 29, 30])
bpy.ops.transform.translate(value=(0, 0, 0.6))
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.unwrap(method='CONFORMAL', fill_holes=False)
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[29])
bpy.ops.uv.pin()
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.minimize_stretch(blend=0.25, iterations=20)
bpy.ops.object.mode_set(mode='OBJECT')
