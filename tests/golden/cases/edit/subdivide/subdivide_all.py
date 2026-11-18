# WP 2.6: mesh.subdivide on the whole cube, one cut: edge midpoints in edge order (each from the
# edge's first vertex), then the face centres in reversed face order (the fill stack), T1.
# golden: startup=factory compare=objects,context,mesh:Cube.001 tier=T1
import bpy
bpy.ops.mesh.primitive_cube_add()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.subdivide()
bpy.data.meshes['Cube.001'].total_vert_sel
