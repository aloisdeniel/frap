# WP 2.4: mesh.select_more / select_less in face and edge select modes (region_extend with
# use_faces in face-only mode, BM_mesh_select_mode_clean after select_less).
# golden: startup=factory compare=objects,context,mesh:Grid tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=6, y_subdivisions=6)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[12])
bpy.ops.mesh.select_more(use_face_step=False)
bpy.ops.mesh.select_more()
bpy.ops.mesh.select_less()
bpy.ops.mesh.select_less(use_face_step=False)
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.frap.select_elements(type='EDGE', indices=[20, 21])
bpy.ops.mesh.select_more()
bpy.ops.mesh.select_less()
bpy.ops.mesh.select_less()
bpy.ops.mesh.select_more()
