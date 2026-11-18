# WP 2.4: edit/select/history.py cut after its line 13,
#   bpy.ops.mesh.select_random(ratio=0.5, seed=9, action='DESELECT')
# so that call's effect reaches the dump (later lines of the full case replace it).
# golden: startup=factory compare=objects,context,mesh:Grid tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=5, y_subdivisions=5)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[6, 7, 12])
bpy.ops.mesh.select_more()
bpy.ops.mesh.select_less(use_face_step=False)
bpy.ops.mesh.select_linked()
bpy.ops.mesh.select_random(ratio=0.5, seed=9, action='DESELECT')
