# WP 2.10: mesh.edge_split(type='EDGE'): one edge of the closed cube adds no vertex (its ends
# stay fan-connected through the other faces); the four edges around the top face separate
# it from the sides (four new vertices); the split edges are selected.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.frap.select_elements(type='EDGE', indices=[0])
bpy.ops.mesh.edge_split()
bpy.data.meshes['Cube'].total_vert_sel
bpy.ops.frap.select_elements(type='FACE', indices=[5])
bpy.ops.mesh.edge_split(type='EDGE')
bpy.data.meshes['Cube'].total_edge_sel
