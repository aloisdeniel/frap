# WP 2.10: mesh.tris_convert_to_quads re-joins a triangulated cube into 6 quads (another face
# order and corner start than the original, so T2), with the default 40° limits; the joined
# quads are added to the selection. Then a triangulated grid with topology_influence, and
# two triangles alone (a face pair: the limits are lifted unless set), and deselect_joined.
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Grid tier=T2
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.quads_convert_to_tris(quad_method='FIXED')
bpy.ops.mesh.tris_convert_to_quads()
bpy.data.meshes['Cube'].total_face_sel
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=3, size=2, location=(4, 0, 0), enter_editmode=True)
bpy.ops.mesh.quads_convert_to_tris(quad_method='FIXED_ALTERNATE')
bpy.ops.frap.select_elements(type='FACE', indices=[0, 1, 2, 3, 4, 5, 6, 7, 8, 9])
bpy.ops.mesh.tris_convert_to_quads(topology_influence=0.6)
bpy.data.meshes['Grid'].total_face_sel
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.tris_convert_to_quads(deselect_joined=True)
bpy.data.meshes['Grid'].total_face_sel
