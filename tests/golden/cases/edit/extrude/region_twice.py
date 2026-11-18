# WP 2.5: two region extrusions in a row reuse the freed slots; the selection history follows
# the newest cap.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1 order=canonical
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[2])
bpy.ops.mesh.extrude_region()
bpy.ops.mesh.extrude_region()
bpy.data.meshes['Cube'].total_vert_sel
bpy.data.meshes['Cube'].total_edge_sel
bpy.data.meshes['Cube'].total_face_sel
