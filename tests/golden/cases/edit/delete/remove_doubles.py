# WP 2.7: mesh.remove_doubles merges the doubles of a cube joined onto itself (Removed 8
# vertices), keeps the selection, and on the plain cube with a huge threshold leaves two
# vertices (clusters are not chained, research App. B.8).
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.primitive_cube_add()
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.remove_doubles()
bpy.data.meshes['Cube'].total_vert_sel
bpy.ops.mesh.remove_doubles()
bpy.ops.mesh.remove_doubles(threshold=3)
