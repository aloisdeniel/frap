# WP 2.5: mesh.inset(thickness=0.2) on the top face: the face is kept and moves inward, new
# vertices take the old corners, four ring quads (V12 E20 F10); UVs are interpolated.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[0])
bpy.ops.mesh.inset(thickness=0.2)
bpy.data.meshes['Cube'].total_vert_sel
bpy.data.meshes['Cube'].total_edge_sel
bpy.data.meshes['Cube'].total_face_sel
