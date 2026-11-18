# WP 2.5: mesh.inset on two adjacent faces insets them as one region (the shared edge is not
# inset: V14 E24 F12).
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[0, 4])
bpy.ops.mesh.inset(thickness=0.2)
bpy.data.meshes['Cube'].total_vert_sel
bpy.data.meshes['Cube'].total_edge_sel
bpy.data.meshes['Cube'].total_face_sel
