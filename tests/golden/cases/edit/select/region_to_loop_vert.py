# WP 2.4: mesh.region_to_loop in vertex mode: two vertices that span no face give no loop
# (outcome only: the next line replaces the selection);
# a single face selected by index gives its four boundary edges.
# golden: startup=factory compare=objects,context,mesh:Grid tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=6, y_subdivisions=6)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 12])
bpy.ops.mesh.region_to_loop()
bpy.ops.frap.select_elements(type='FACE', indices=[0])
bpy.ops.mesh.region_to_loop()
