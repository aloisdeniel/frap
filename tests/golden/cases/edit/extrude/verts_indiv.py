# WP 2.5: mesh.extrude_verts_indiv on two vertices of one edge: two wire edges; the originals
# are deselected (the edge too, by the flush) and the copies selected; the history follows.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 2])
bpy.ops.mesh.extrude_verts_indiv()
bpy.data.meshes['Cube'].total_vert_sel
bpy.data.meshes['Cube'].total_edge_sel
bpy.data.meshes['Cube'].total_face_sel
