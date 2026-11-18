# WP 2.4: mesh.hide (selected / unselected) in vertex, edge and face mode, hidden elements
# skipped by select_all and select_more, CANCELLED when there is nothing to hide, and
# mesh.reveal with select=True / False.
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
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.frap.select_elements(type='EDGE', indices=[4, 5])
bpy.ops.mesh.hide(unselected=True)
bpy.ops.mesh.select_more()
bpy.ops.mesh.reveal()
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[4])
bpy.ops.mesh.hide()
bpy.ops.frap.select_elements(type='FACE', indices=[0, 8])
bpy.ops.mesh.hide(unselected=True)
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.hide(unselected=True)
bpy.ops.mesh.reveal()
