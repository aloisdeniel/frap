# WP 2.10: mesh.flip_quad_tessellation starts every selected quad one corner later (its
# tessellation diagonal moves from corners 0-2 to 1-3); triangles and n-gons are left alone.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[0])
bpy.ops.mesh.quads_convert_to_tris(quad_method='FIXED')
bpy.ops.frap.select_elements(type='FACE', indices=[0, 1, 2, 5])
bpy.ops.mesh.flip_quad_tessellation()
bpy.ops.mesh.flip_quad_tessellation()
bpy.ops.frap.select_elements(type='FACE', indices=[3])
bpy.ops.mesh.flip_quad_tessellation()
bpy.data.meshes['Cube'].total_face_sel
