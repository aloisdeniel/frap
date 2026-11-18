# WP 2.4: edit/select/loop_select.py cut after its line 15,
#   bpy.ops.mesh.loop_select(object_index=0, edge_index=0, toggle=True)
# so that call's effect reaches the dump (later lines of the full case replace it).
# golden: startup=factory compare=objects,context,mesh:Grid tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=6, y_subdivisions=6)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.loop_select(edge_index=7)
bpy.ops.mesh.loop_select(object_index=0, edge_index=7)
bpy.ops.mesh.loop_select(object_index=0, edge_index=7, vert_index=3)
bpy.ops.mesh.edgering_select(object_index=0, edge_index=7, extend=True)
bpy.ops.mesh.loop_select(object_index=0, edge_index=0, toggle=True)
