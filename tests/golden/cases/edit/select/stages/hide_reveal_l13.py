# WP 2.4: edit/select/hide_reveal.py cut after its line 13,
#   bpy.ops.mesh.reveal(select=False)
# then select_all(INVERT), which skips hidden elements: the hide state reaches the dump.
# golden: startup=factory compare=objects,context,mesh:Grid tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=4)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[5, 6])
bpy.ops.mesh.hide()
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.hide()
bpy.ops.mesh.reveal(select=False)
bpy.ops.mesh.select_all(action='INVERT')
