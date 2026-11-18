# WP 2.5: mesh.extrude_region / extrude_context in face mode with vertices but no face selected:
# nothing to extrude; the operators finish without a report and change nothing.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 1])
bpy.ops.mesh.extrude_region()
bpy.ops.mesh.extrude_context()
bpy.data.meshes['Cube'].total_vert_sel
bpy.data.meshes['Cube'].total_edge_sel
bpy.data.meshes['Cube'].total_face_sel
