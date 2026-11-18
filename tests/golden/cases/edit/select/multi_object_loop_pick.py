# WP 2.4: mesh.loop_select in multi-object Edit Mode picks by object_index: object 1 without
# extend replaces the (all-selected) selection of both meshes with one loop, then object 0
# with extend adds a loop on the other mesh.
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Grid tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=4, location=(3, 0, 0))
bpy.data.objects['Cube'].select_set(True)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.loop_select(object_index=1, edge_index=4)
bpy.ops.mesh.loop_select(object_index=0, edge_index=5, extend=True)
