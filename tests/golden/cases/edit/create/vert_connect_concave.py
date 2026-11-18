# WP 2.8: mesh.vert_connect_concave leaves convex faces alone (FINISHED, nothing changes);
# splitting concave faces triangulates through WP 2.10's `triangulate` (case to add at
# integration).
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.vert_connect_concave()
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.vert_connect_concave()
