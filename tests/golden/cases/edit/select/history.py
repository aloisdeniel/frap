# WP 2.4: the selection history (active element) through the selection operators: kept (minus
# deselected entries) by select_more/less, select_linked, select_random and the multi loop
# selectors, cleared by region_to_loop, loop_to_region and non-extending selectors, set by
# loop_select in edge mode.
# golden: startup=factory compare=objects,context,mesh:Grid tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=5, y_subdivisions=5)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[6, 7, 12])
bpy.ops.mesh.select_more()
bpy.ops.mesh.select_less(use_face_step=False)
bpy.ops.mesh.select_linked()
bpy.ops.mesh.select_random(ratio=0.5, seed=9, action='DESELECT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.frap.select_elements(type='EDGE', indices=[5, 9])
bpy.ops.mesh.select_edge_loop_multi()
bpy.ops.mesh.select_face_by_sides(number=4, extend=False)
bpy.ops.frap.select_elements(type='EDGE', indices=[12, 13])
bpy.ops.mesh.loop_select(object_index=0, edge_index=20, extend=True)
bpy.ops.mesh.select_non_manifold()
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[6, 7])
bpy.ops.mesh.region_to_loop()
