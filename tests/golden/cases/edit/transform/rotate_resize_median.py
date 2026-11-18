# WP 2.12: rotate and resize about the median point of the selection (world space), with and
# without constraints, on a transformed object.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1 tol=1e-5
import bpy
bpy.data.objects['Cube'].location = (1, 2, 0.5)
bpy.data.objects['Cube'].rotation_euler = (0.3, 0, 0.7)
bpy.data.objects['Cube'].scale = (2, 1, 0.5)

bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[2])
bpy.ops.transform.rotate(value=0.4, orient_axis='Z')
bpy.ops.transform.rotate(value=0.3, orient_axis='X', constraint_axis=(True, False, False))
bpy.ops.transform.resize(value=(1.5, 0.5, 2))
bpy.ops.transform.resize(value=(0.5, 1, 1), orient_type='LOCAL', constraint_axis=(True, False, False))
bpy.ops.transform.rotate(value=0.2, orient_axis='Y', orient_type='LOCAL')
