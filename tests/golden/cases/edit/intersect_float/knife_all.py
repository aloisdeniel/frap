# WP 8.6: mesh.intersect(solver='FLOAT', separate_mode='ALL'): a tilted plane added in Edit Mode
# (selected) cuts the default cube; the mesh is read in Edit Mode (selection included).
# golden: startup=factory compare=objects,context,mesh:Cube tier=T2 tol=1e-4
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.primitive_plane_add(size=3, location=(0, 0, 0.25), rotation=(0.2, 0.1, 0))
bpy.ops.mesh.intersect(solver='FLOAT', separate_mode='ALL')
