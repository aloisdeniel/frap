# WP 2.8: mesh.fill_grid on a closed loop uses the active vertex (last selected) as the grid
# corner when it is on the loop; the boundary of a 4x2 grid hole (corners detected by angle)
# with the default span.
# golden: startup=empty compare=objects,context,mesh:Circle tier=T1
import bpy
bpy.ops.mesh.primitive_circle_add(vertices=12, radius=1, fill_type='NOTHING')
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[3], action='ADD')
bpy.ops.mesh.fill_grid()
bpy.data.meshes['Circle'].total_face_sel
bpy.ops.mesh.primitive_circle_add(vertices=10, radius=1, fill_type='NOTHING', location=(3, 0, 0), rotation=(0.4, 0, 0))
bpy.ops.mesh.fill_grid(span=3)
