# WP 2.5: mesh.extrude_repeat on one vertex: a chain of wire edges.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[0])
bpy.ops.mesh.extrude_repeat(steps=3, offset=(0.25, 0, 0))
bpy.data.meshes['Cube'].total_vert_sel
bpy.data.meshes['Cube'].total_edge_sel
bpy.data.meshes['Cube'].total_face_sel
