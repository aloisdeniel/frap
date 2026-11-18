# WP 2.8 (needs 2.10 split, 2.9 reverse_faces, 2.7 weld_verts): fill_grid with faces selected
# (re-grids the selected region).
# golden: startup=empty compare=objects,context,mesh:Grid tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=4, size=2)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[0, 1, 2, 4, 5, 6])
bpy.ops.mesh.fill_grid()
bpy.data.meshes['Grid'].total_face_sel
