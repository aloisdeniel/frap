# WP 2.5: mesh.extrude_region with everything selected: no face borders an unselected one, so
# the originals are kept and flipped and no side face is made; the copy is selected.
# Deterministic: compared index by index.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.extrude_region()
bpy.data.meshes['Cube'].total_vert_sel
bpy.data.meshes['Cube'].total_edge_sel
bpy.data.meshes['Cube'].total_face_sel
