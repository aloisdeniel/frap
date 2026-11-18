# WP 2.7: mesh.dissolve_degenerate collapses edges shorter than the threshold and clips
# corners whose sides fold onto each other; a clean mesh reports nothing.
# golden: startup=factory compare=objects,context,mesh:Grid tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=4)
bpy.data.meshes['Grid'].vertices[6].co = (-0.5, -0.99999, 0)
bpy.data.meshes['Grid'].vertices[12].co = (0.00001, 0.00001, 0)
bpy.data.meshes['Grid'].vertices[19].co = (0.5, 0.5, 0)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.dissolve_degenerate(threshold=0.001)
bpy.ops.mesh.dissolve_degenerate()
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.dissolve_degenerate(threshold=0.3)
