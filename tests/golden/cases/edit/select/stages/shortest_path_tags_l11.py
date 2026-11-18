# WP 2.4: edit/select/shortest_path_tags.py cut after its line 11,
#   bpy.ops.mesh.shortest_path_select()
# so that call's effect reaches the dump (later lines of the full case replace it).
# golden: startup=factory compare=objects,context,mesh:Grid tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=5, y_subdivisions=5)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.mesh.select_random(ratio=0.05, seed=2)
bpy.ops.mesh.shortest_path_select()
