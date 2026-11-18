# WP 2.10: mesh.vertices_smooth: the default factor 0 is a no-op; factor 0.5 on the whole
# cube moves every coordinate to ±2/3; repeat and axis limits on part of a grid; with
# nothing selected: WARNING "No selected vertex" and CANCELLED.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.vertices_smooth()
bpy.ops.mesh.vertices_smooth(factor=0.5)
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=4, size=2, location=(4, 0, 0), rotation=(0.2, 0.3, 0))
bpy.ops.frap.select_elements(type='VERT', indices=[13, 14, 18, 19, 20, 24])
bpy.ops.mesh.vertices_smooth(factor=0.8, repeat=3, zaxis=False)
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.vertices_smooth(factor=0.5)
