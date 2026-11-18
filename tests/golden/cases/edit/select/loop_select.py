# WP 2.4: mesh.loop_select / edgering_select by index (Blender's exec: object_index among the
# objects in Edit Mode, edge_index): replace, extend, deselect, toggle; picking a selected
# boundary loop again selects the whole boundary; the edge (edge mode) or vert_index /
# face_index become the active element; without object_index, or with an edge index out of
# range, the operator is CANCELLED.
# golden: startup=factory compare=objects,context,mesh:Grid tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=6, y_subdivisions=6)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.loop_select(edge_index=7)
bpy.ops.mesh.loop_select(object_index=0, edge_index=7)
bpy.data.meshes['Grid'].total_edge_sel
bpy.ops.mesh.loop_select(object_index=0, edge_index=7, vert_index=3)
bpy.data.meshes['Grid'].total_edge_sel
bpy.ops.mesh.edgering_select(object_index=0, edge_index=7, extend=True)
bpy.data.meshes['Grid'].total_edge_sel
bpy.ops.mesh.loop_select(object_index=0, edge_index=0, toggle=True)
bpy.ops.mesh.loop_select(object_index=0, edge_index=30, deselect=True)
bpy.ops.mesh.loop_select(object_index=0, edge_index=999)
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.mesh.loop_select(object_index=0, edge_index=12)
bpy.ops.mesh.edgering_select(object_index=0, edge_index=12, extend=True, face_index=7)
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.mesh.loop_select(object_index=0, edge_index=12, face_index=3)
bpy.ops.mesh.edgering_select(object_index=0, edge_index=20, extend=True)
