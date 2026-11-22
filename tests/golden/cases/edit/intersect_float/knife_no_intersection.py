# WP 8.6: mesh.intersect(solver='FLOAT') with a selection far from the rest: WARNING 'No
# intersections found' and nothing changes.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T2 tol=1e-4
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.primitive_plane_add(size=1, location=(0, 0, 3))
bpy.ops.mesh.intersect(solver='FLOAT')
