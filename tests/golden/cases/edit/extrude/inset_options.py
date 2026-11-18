# WP 2.5: mesh.inset with use_even_offset=False, use_relative_offset, use_edge_rail and no UV
# interpolation on three faces around a corner.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[0, 4, 5])
bpy.ops.mesh.inset(thickness=0.1, depth=0.05, use_even_offset=False, use_relative_offset=True, use_edge_rail=True, use_interpolate=False)
bpy.data.meshes['Cube'].total_vert_sel
bpy.data.meshes['Cube'].total_edge_sel
bpy.data.meshes['Cube'].total_face_sel
