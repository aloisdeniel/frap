# WP 2.5: mesh.extrude_region in edge mode without a selected face extrudes each edge alone
# (extrude_edge_only); the new edge and its vertices are selected.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.frap.select_elements(type='EDGE', indices=[2])
bpy.ops.mesh.extrude_region()
bpy.data.meshes['Cube'].total_vert_sel
bpy.data.meshes['Cube'].total_edge_sel
bpy.data.meshes['Cube'].total_face_sel
