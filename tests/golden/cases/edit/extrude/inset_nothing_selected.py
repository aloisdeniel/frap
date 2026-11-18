# WP 2.5: mesh.inset with nothing selected is cancelled; with vertices but no face it finishes and
# deselects everything.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.inset(thickness=0.1)
bpy.ops.frap.select_elements(type='VERT', indices=[0])
bpy.ops.mesh.inset(thickness=0.1)
bpy.data.meshes['Cube'].total_vert_sel
bpy.data.meshes['Cube'].total_edge_sel
bpy.data.meshes['Cube'].total_face_sel
