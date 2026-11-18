# WP 2.5: mesh.extrude_verts_indiv repeated on the end of a wire chain keeps the chain's edge
# direction (the copy -> original swap of extrude_vert_indiv).
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[5])
bpy.ops.mesh.extrude_verts_indiv()
bpy.ops.mesh.extrude_verts_indiv()
bpy.ops.mesh.extrude_verts_indiv()
bpy.data.meshes['Cube'].total_vert_sel
bpy.data.meshes['Cube'].total_edge_sel
bpy.data.meshes['Cube'].total_face_sel
