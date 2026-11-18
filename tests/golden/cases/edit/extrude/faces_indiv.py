# WP 2.5: mesh.extrude_faces_indiv on two adjacent faces: each face extruded alone
# (V16 E28 F14), the copies selected, the history cleared.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[0, 4])
bpy.ops.mesh.extrude_faces_indiv()
bpy.data.meshes['Cube'].total_vert_sel
bpy.data.meshes['Cube'].total_edge_sel
bpy.data.meshes['Cube'].total_face_sel
