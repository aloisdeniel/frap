# WP 2.12: orient_type='NORMAL' from the selection: faces, three vertices, an edge, two
# vertices, one vertex, many vertices.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1 tol=1e-5
import bpy
bpy.data.objects['Cube'].rotation_euler = (0.3, 0.2, 0.1)
bpy.data.objects['Cube'].scale = (1, 1.5, 0.8)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[1, 2])
bpy.ops.transform.translate(value=(0.1, 0.2, 0.3), orient_type='NORMAL')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 1, 2])
bpy.ops.transform.translate(value=(0.1, 0.2, 0.3), orient_type='NORMAL')
bpy.ops.frap.select_elements(type='EDGE', indices=[4])
bpy.ops.transform.rotate(value=0.4, orient_axis='Y', orient_type='NORMAL')
bpy.ops.frap.select_elements(type='VERT', indices=[5, 6])
bpy.ops.transform.translate(value=(0.3, 0.1, 0), orient_type='NORMAL')
bpy.ops.frap.select_elements(type='VERT', indices=[3])
bpy.ops.transform.translate(value=(0.1, 0.2, 0.3), orient_type='NORMAL')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 3, 4, 7])
bpy.ops.transform.resize(value=(1, 1, 1.5), orient_type='NORMAL')
