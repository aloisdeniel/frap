# WP 2.12: transform.edge_slide of edge chains and loops: two open rows of a grid (oriented
# alike by the 3D View), the bottom ring of a cylinder sliding up the sides and into the cap,
# an invalid selection (cancelled).
# golden: startup=factory compare=objects,context,mesh:Grid tier=T1 tol=1e-5
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=4, size=3)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[5, 6, 7, 8, 9, 16, 17, 18])
bpy.ops.transform.edge_slide(value=0.4)
bpy.ops.transform.edge_slide(value=-0.6, use_even=True)
bpy.ops.transform.edge_slide(value=0.3, use_even=True, flipped=True)
bpy.ops.frap.select_elements(type='VERT', indices=[6, 7, 11, 12])
bpy.ops.transform.edge_slide(value=0.5)
bpy.ops.mesh.primitive_cylinder_add(vertices=8, location=(4, 0, 0))
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[25, 27, 29, 31, 33, 35, 37, 39])
bpy.ops.transform.edge_slide(value=0.3)
bpy.ops.transform.edge_slide(value=-0.4)
