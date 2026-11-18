# WP 2.4: edit/select/shortest_path.py cut after its line 24,
#   bpy.ops.mesh.shortest_path_select(use_fill=True)
# so that call's effect reaches the dump (later lines of the full case replace it).
# golden: startup=factory compare=objects,context,mesh:Grid tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=6, y_subdivisions=6)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 35])
bpy.ops.mesh.shortest_path_select()
bpy.ops.frap.select_elements(type='VERT', indices=[2, 33])
bpy.ops.mesh.shortest_path_select(use_topology_distance=True, use_face_step=True)
bpy.ops.frap.select_elements(type='VERT', indices=[6, 11])
bpy.ops.mesh.shortest_path_select(skip=1, nth=1)
bpy.ops.frap.select_elements(type='VERT', indices=[7, 28])
bpy.ops.mesh.shortest_path_select(use_fill=True)
bpy.ops.frap.select_elements(type='VERT', indices=[9])
bpy.ops.mesh.shortest_path_select()
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.frap.select_elements(type='EDGE', indices=[0, 50])
bpy.ops.mesh.shortest_path_select()
bpy.ops.frap.select_elements(type='EDGE', indices=[3, 40])
bpy.ops.mesh.shortest_path_select(use_fill=True)
