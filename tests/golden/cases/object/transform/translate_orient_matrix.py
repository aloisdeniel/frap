# WP 1.21 (T0), transform.translate with orient_matrix: used when orient_type (default GLOBAL)
# equals orient_matrix_type; nested tuples are the matrix columns.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.ops.transform.translate(value=(1, 0, 0), orient_matrix=((0, 1, 0), (-1, 0, 0), (0, 0, 1)), orient_matrix_type='GLOBAL')
bpy.ops.transform.translate(value=(0, 0, 1), orient_type='LOCAL', orient_matrix=((0, 1, 0), (-1, 0, 0), (0, 0, 1)), orient_matrix_type='GLOBAL')
