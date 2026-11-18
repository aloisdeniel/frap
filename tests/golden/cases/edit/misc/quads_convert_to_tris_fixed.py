# WP 2.10: mesh.quads_convert_to_tris with the index-exact quad methods (T1): FIXED splits
# corners 0-2, FIXED_ALTERNATE 1-3, SHORTEST/LONGEST_DIAGONAL compare the diagonals of a
# sheared quad; each face's last triangle keeps its slot, the others are appended; the new
# triangles are selected.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[0, 5])
bpy.ops.mesh.quads_convert_to_tris(quad_method='FIXED')
bpy.data.meshes['Cube'].total_face_sel
bpy.ops.frap.select_elements(type='FACE', indices=[1])
bpy.ops.mesh.quads_convert_to_tris(quad_method='FIXED_ALTERNATE')
bpy.ops.mesh.primitive_grid_add(x_subdivisions=2, y_subdivisions=2, size=1, location=(3, 0, 0), rotation=(0.3, 0.2, 0.1))
bpy.ops.mesh.quads_convert_to_tris(quad_method='SHORTEST_DIAGONAL')
bpy.ops.mesh.primitive_plane_add(size=1, location=(0, 3, 0), rotation=(0, 0.5, 0.8), scale=(1, 2, 1))
bpy.ops.mesh.quads_convert_to_tris(quad_method='LONGEST_DIAGONAL')
bpy.data.meshes['Cube'].total_face_sel
