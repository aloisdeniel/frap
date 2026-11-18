# WP 2.10: mesh.edge_split(type='VERT'): every face and edge around the selected vertices is
# split off (one vertex per face corner) and the corners of selected vertices stay selected.
# A plane merged into the edit mesh with two selected corners is split too.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[0])
bpy.ops.mesh.edge_split(type='VERT')
bpy.data.meshes['Cube'].total_vert_sel
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.primitive_plane_add(size=1, location=(3, 0, 0))
bpy.ops.frap.select_elements(type='VERT', indices=[11, 12])
bpy.ops.mesh.edge_split(type='VERT')
bpy.data.meshes['Cube'].total_vert_sel
