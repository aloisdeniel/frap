# WP 2.4: edit/select/more_less.py cut after its line 12,
#   bpy.ops.mesh.select_less()
# so that call's effect reaches the dump (later lines of the full case replace it).
# golden: startup=factory compare=objects,context,mesh:Grid tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=6, y_subdivisions=6)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[14])
bpy.ops.mesh.select_more(use_face_step=False)
bpy.ops.mesh.select_more()
bpy.ops.mesh.select_less(use_face_step=False)
bpy.ops.mesh.select_less()
