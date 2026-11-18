# WP 2.11: vertex bevel of cube vertex 0 (verified counts V10 E15 F7): the new triangle and
# its vertices and edges are selected.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[0])
bpy.ops.mesh.bevel(offset=0.2, affect='VERTICES')
bpy.data.meshes['Cube'].total_edge_sel
