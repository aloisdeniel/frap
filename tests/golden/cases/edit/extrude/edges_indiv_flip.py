# WP 2.5: mesh.extrude_edges_indiv(use_normal_flip=True) winds the side quad the other way.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.frap.select_elements(type='EDGE', indices=[4])
bpy.ops.mesh.extrude_edges_indiv(use_normal_flip=True)
bpy.data.meshes['Cube'].total_vert_sel
bpy.data.meshes['Cube'].total_edge_sel
bpy.data.meshes['Cube'].total_face_sel
