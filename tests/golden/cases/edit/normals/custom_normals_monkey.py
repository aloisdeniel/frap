# WP 2.9: custom split normals on Suzanne (smooth, with sharp edges from
# set_sharpness_by_angle): set_normals_from_faces on a face selection, then flip_normals
# re-encodes them in the new fan spaces, normals_make_consistent(inside=True) carries them,
# and only_clnors flips the selected fans (vertex + face select mode).
# golden: startup=factory compare=objects,context,mesh:Suzanne tier=T1
import bpy
bpy.ops.mesh.primitive_monkey_add()
bpy.ops.object.shade_smooth()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.set_sharpness_by_angle(angle=1.0)
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[10, 11, 12, 13, 50, 51, 52, 200, 201, 202, 203])
bpy.ops.mesh.set_normals_from_faces()
bpy.ops.frap.select_elements(type='FACE', indices=[11, 12, 50, 201])
bpy.ops.mesh.flip_normals()
bpy.ops.frap.select_elements(type='FACE', indices=[10, 11, 12, 13, 14, 15])
bpy.ops.mesh.normals_make_consistent(inside=True)
bpy.ops.mesh.select_mode(type='VERT', use_extend=True)
bpy.ops.frap.select_elements(type='VERT', indices=[5, 6, 7, 100])
bpy.ops.mesh.flip_normals(only_clnors=True)
