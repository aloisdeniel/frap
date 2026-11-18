# WP 2.5: mesh.extrude_region / extrude_context / extrude_verts_indiv / extrude_edges_indiv /
# extrude_faces_indiv with nothing selected finish without changing anything.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.extrude_region()
bpy.ops.mesh.extrude_context()
bpy.ops.mesh.extrude_verts_indiv()
bpy.ops.mesh.extrude_edges_indiv()
bpy.ops.mesh.extrude_faces_indiv()
bpy.ops.mesh.extrude_repeat(steps=2, offset=(0, 0, 1))
bpy.data.meshes['Cube'].total_vert_sel
bpy.data.meshes['Cube'].total_edge_sel
bpy.data.meshes['Cube'].total_face_sel
