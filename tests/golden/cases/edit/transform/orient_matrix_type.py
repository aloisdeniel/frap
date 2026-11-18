# WP 2.12: orient_matrix is used when orient_matrix_type is set and equals the orientation (an
# unset orient_type takes orient_matrix_type), or when neither type is set; with orient_type set
# and orient_matrix_type unset, the matrix is ignored.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1 tol=1e-5
import bpy
bpy.ops.transform.rotate(value=0.5, orient_axis='Z')
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 1, 4])
bpy.ops.transform.rotate(value=0.3, orient_axis='X', orient_matrix=((0, 1, 0), (-1, 0, 0), (0, 0, 1)), orient_matrix_type='LOCAL')
bpy.ops.transform.rotate(value=0.3, orient_axis='X', orient_matrix=((0, 1, 0), (-1, 0, 0), (0, 0, 1)), orient_matrix_type='LOCAL', constraint_axis=(True, False, False))
bpy.ops.transform.translate(value=(0.1, 0.2, 0), orient_matrix=((0, 1, 0), (-1, 0, 0), (0, 0, 1)), constraint_axis=(True, False, False))
bpy.ops.transform.translate(value=(0.1, 0.2, 0), orient_type='GLOBAL', orient_matrix=((0, 1, 0), (-1, 0, 0), (0, 0, 1)), constraint_axis=(True, False, False))
bpy.ops.transform.translate(value=(0.1, 0.2, 0), orient_type='LOCAL', orient_matrix=((0, 1, 0), (-1, 0, 0), (0, 0, 1)), orient_matrix_type='LOCAL', constraint_axis=(False, True, False))
bpy.ops.transform.resize(value=(1.5, 1, 1), orient_type='NORMAL', orient_matrix=((0, 1, 0), (-1, 0, 0), (0, 0, 1)), orient_matrix_type='GLOBAL', constraint_axis=(True, False, False))
bpy.ops.object.mode_set(mode='OBJECT')
