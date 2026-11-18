# WP 2.12: the other orientations in Edit Mode: GIMBAL (Euler and quaternion active object, the
# latter falls back to its parent's axes), PARENT, VIEW, CURSOR, and orient_matrix.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1 tol=1e-5
import bpy
bpy.data.objects['Cube'].rotation_euler = (0.3, 0.4, 0.5)
bpy.data.objects['Cube'].parent = bpy.data.objects['Light']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 4])
bpy.ops.transform.translate(value=(0.2, 0.1, 0.3), orient_type='GIMBAL')
bpy.ops.transform.translate(value=(0.2, 0.1, 0.3), orient_type='PARENT')
bpy.ops.transform.translate(value=(0.2, 0.1, 0.3), orient_type='VIEW')
bpy.context.scene.cursor.rotation_euler = (0, 0.7, 0)
bpy.ops.transform.rotate(value=0.4, orient_axis='Y', orient_type='CURSOR')
bpy.ops.transform.resize(value=(1.5, 1, 1), orient_matrix=((0, 1, 0), (-1, 0, 0), (0, 0, 1)), orient_matrix_type='GLOBAL')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Cube'].rotation_mode = 'QUATERNION'
bpy.data.objects['Cube'].rotation_quaternion = (0.9, 0.1, 0.3, 0.2)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 4])
bpy.ops.transform.translate(value=(0.2, 0.1, 0.3), orient_type='GIMBAL')
