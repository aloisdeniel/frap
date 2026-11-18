# WP 2.4: edit/select/hide_reveal.py cut after its line 9,
#   bpy.ops.mesh.hide()
# so that call's effect reaches the dump (later lines of the full case replace it).
# golden: startup=factory compare=objects,context,mesh:Grid tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=4)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[5, 6])
bpy.ops.mesh.hide()
