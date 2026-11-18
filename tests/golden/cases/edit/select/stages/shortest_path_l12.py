# WP 2.4: edit/select/shortest_path.py cut after its line 12,
#   bpy.ops.mesh.shortest_path_select(use_topology_distance=True, use_face_step=True)
# so that call's effect reaches the dump (later lines of the full case replace it).
# golden: startup=factory compare=objects,context,mesh:Grid tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=6, y_subdivisions=6)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 35])
bpy.ops.mesh.shortest_path_select()
bpy.ops.frap.select_elements(type='VERT', indices=[2, 33])
bpy.ops.mesh.shortest_path_select(use_topology_distance=True, use_face_step=True)
