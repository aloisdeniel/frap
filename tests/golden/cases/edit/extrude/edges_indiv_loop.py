# WP 2.5: mesh.extrude_edges_indiv on the four edges of the top face: four quads around a copied
# loop (compared order-free: Blender's side-quad order is run-dependent).
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1 order=canonical
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.frap.select_elements(type='EDGE', indices=[2, 7, 9, 10])
bpy.ops.mesh.extrude_edges_indiv()
bpy.data.meshes['Cube'].total_vert_sel
bpy.data.meshes['Cube'].total_edge_sel
bpy.data.meshes['Cube'].total_face_sel
