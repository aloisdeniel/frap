# WP 2.11: crossing edges in a grid (a 'pipe' at the crossing vertices: beveled edges,
# two of them collinear), two segments; boundary edges are not beveled.
# golden: startup=factory compare=objects,context,mesh:Grid tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=4)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='EDGE', indices=[6, 9, 11, 14, 17, 20])
bpy.ops.mesh.bevel(offset=0.1, segments=2)
