# WP 2.7: mesh.merge(type='COLLAPSE') collapses each connected group of selected edges to
# its centre; outside vertex select mode everything is deselected afterwards.
# golden: startup=factory compare=objects,context,mesh:Grid tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=4)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 1, 5, 6, 18, 19])
bpy.ops.mesh.merge(type='COLLAPSE')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.frap.select_elements(type='EDGE', indices=[3])
bpy.ops.mesh.merge(type='COLLAPSE')
