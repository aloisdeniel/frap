# WP 2.4: edit/select/more_less_faces.py cut after its line 9,
#   bpy.ops.mesh.select_more(use_face_step=False)
# so that call's effect reaches the dump (later lines of the full case replace it).
# golden: startup=factory compare=objects,context,mesh:Grid tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=6, y_subdivisions=6)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[12])
bpy.ops.mesh.select_more(use_face_step=False)
