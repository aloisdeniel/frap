# WP 2.4: mesh.loop_to_region selects the smaller region inside the loop made by
# region_to_loop (the original faces).
# golden: startup=factory compare=objects,context,mesh:Grid tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=6, y_subdivisions=6)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[6, 7, 8, 11, 12, 13])
bpy.ops.mesh.region_to_loop()
bpy.ops.mesh.loop_to_region()
