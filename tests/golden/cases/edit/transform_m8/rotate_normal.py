# WP 8.3.5 (T1), transform.rotate_normal: the custom normals of the selection turned about
# +orient_axis (the custom_normal attribute created), the constrained axis, LOCAL and NORMAL
# orientations, vertex and edge selections, smooth fans; the poll needs Edit Mode.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1 tol=1e-4
import bpy
bpy.ops.transform.rotate_normal(value=0.3)
bpy.data.objects['Cube'].rotation_euler = (0.3, 0.2, 0.1)
bpy.ops.object.shade_smooth()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[1])
bpy.ops.transform.rotate_normal(value=0.3)
bpy.ops.frap.select_elements(type='FACE', indices=[2, 3])
bpy.ops.transform.rotate_normal(value=-0.5, orient_axis='X')
bpy.ops.frap.select_elements(type='VERT', indices=[0])
bpy.ops.transform.rotate_normal(value=0.4, constraint_axis=(False, True, False))
bpy.ops.frap.select_elements(type='EDGE', indices=[5])
bpy.ops.transform.rotate_normal(value=0.6, orient_type='LOCAL', orient_axis='Y')
bpy.ops.frap.select_elements(type='VERT', indices=[6, 7])
bpy.ops.transform.rotate_normal(value=0.2, orient_type='NORMAL', orient_axis='X')
bpy.ops.frap.select_elements(type='VERT', indices=[])
bpy.ops.transform.rotate_normal(value=0.2)
