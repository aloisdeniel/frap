# WP 8.2: every cube edge with four segments (even): the cube-corner vertex mesh
# (`tri_corner_adj_vmesh` / `make_cube_corner_adj_vmesh`), no center ngon.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.bevel(offset=0.25, segments=4)
