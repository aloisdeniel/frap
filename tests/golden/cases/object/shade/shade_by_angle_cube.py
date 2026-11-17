# WP 1.21 (T1), object.shade_smooth_by_angle on the cube: 30 degrees tags all 12 edges; 1.6 rad
# keeps only the existing tag; without keep_sharp_edges none.
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Cube.001,mesh:Cube.002 tier=T1
import bpy
bpy.ops.object.shade_smooth_by_angle()
bpy.ops.mesh.primitive_cube_add(location=(3, 0, 0))
bpy.data.meshes['Cube.001'].edges[0].use_edge_sharp = True
bpy.ops.object.shade_smooth_by_angle(angle=1.6)
bpy.ops.mesh.primitive_cube_add(location=(6, 0, 0))
bpy.data.meshes['Cube.002'].edges[0].use_edge_sharp = True
bpy.ops.object.shade_smooth_by_angle(angle=1.6, keep_sharp_edges=False)
