# WP 2.12: connected proportional editing measures geodesic distances over the surface
# (across faces and along edges), unconnected geometry stays.
# golden: startup=factory compare=objects,context,mesh:Grid tier=T1 tol=1e-5
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=7, y_subdivisions=7, size=3, rotation=(0.4, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.primitive_plane_add(size=1, location=(0.2, 0.3, 0.4))
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[24])
bpy.ops.transform.translate(value=(0, 0.6, 0), use_proportional_edit=True, use_proportional_connected=True, proportional_size=1.5, proportional_edit_falloff='LINEAR')
bpy.ops.transform.rotate(value=0.5, orient_axis='X', use_proportional_edit=True, use_proportional_connected=True, proportional_size=1.0)
