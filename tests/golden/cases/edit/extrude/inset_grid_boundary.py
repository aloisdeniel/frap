# WP 2.5: mesh.inset on a grid's inner faces and on faces at the open boundary, with and without
# use_boundary.
# golden: startup=empty compare=objects,context,mesh:Grid tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=3, size=2)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[0, 1])
bpy.ops.mesh.inset(thickness=0.05)
bpy.ops.frap.select_elements(type='FACE', indices=[4])
bpy.ops.mesh.inset(thickness=0.05, use_boundary=False)
bpy.data.meshes['Grid'].total_face_sel
