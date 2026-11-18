# WP 2.5: mesh.inset with use_edge_rail on an L-shaped region of grid faces (vertices slide
# along the existing edges where both inset edges' faces are adjacent).
# golden: startup=empty compare=objects,context,mesh:Grid tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=4, size=2)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[0, 1, 2, 3, 6])
bpy.ops.mesh.inset(thickness=0.1, use_edge_rail=True, use_boundary=False)
