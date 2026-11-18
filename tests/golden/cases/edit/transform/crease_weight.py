# WP 2.12: edge_crease, edge_bevelweight and vert_crease add the value to the selected
# elements (results clamped to 0..1, the value to at most 1) and create the attribute when missing;
# proportional editing is not offered by these operators.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1 tol=1e-5
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='EDGE', indices=[0, 3, 7])
bpy.ops.transform.edge_crease(value=0.4)
bpy.ops.transform.edge_crease(value=0.8)
bpy.ops.transform.edge_bevelweight(value=0.3)
bpy.ops.frap.select_elements(type='EDGE', indices=[3])
bpy.ops.transform.edge_crease(value=-0.5)
bpy.ops.transform.edge_bevelweight(value=5)
bpy.ops.frap.select_elements(type='VERT', indices=[2, 6])
bpy.ops.transform.vert_crease(value=0.6)
bpy.ops.transform.vert_crease(value=-0.1)
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.transform.edge_crease(value=0.5)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.transform.edge_crease(value=0.5)
