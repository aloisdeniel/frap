# WP 2.7: mesh.edge_collapse collapses each connected group of selected edges to the mean of
# its edges' endpoints (UVs follow).
# golden: startup=factory compare=objects,context,mesh:Grid tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=4, calc_uvs=True)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[6, 7, 8])
bpy.ops.frap.select_elements(type='VERT', indices=[20, 21], action='ADD')
bpy.ops.mesh.edge_collapse()
