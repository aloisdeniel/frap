# WP 2.12: edge slides whose direction depends on the 3D View: two loops on opposite sides of a
# rotated cylinder (one hidden behind the mesh), and single_side.
# golden: startup=factory compare=objects,context,mesh:Cylinder tier=T1 tol=1e-5
import bpy
bpy.ops.mesh.primitive_cylinder_add(vertices=12, rotation=(0.5, 0.3, 0.2))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='EDGE', indices=[1, 7, 13, 19])
bpy.ops.transform.edge_slide(value=0.4)
bpy.ops.transform.edge_slide(value=-0.2, use_even=True)
bpy.ops.transform.edge_slide(value=0.5, single_side=True)
