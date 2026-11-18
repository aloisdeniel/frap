# WP 2.5: mesh.extrude_region in vertex mode with one selected vertex extrudes the vertex alone
# (a wire edge to its copy), as extrude_verts_indiv; the copy is selected and active.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[3])
bpy.ops.mesh.extrude_region()
bpy.data.meshes['Cube'].total_vert_sel
bpy.data.meshes['Cube'].total_edge_sel
bpy.data.meshes['Cube'].total_face_sel
