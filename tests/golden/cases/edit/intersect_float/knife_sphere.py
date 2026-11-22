# WP 8.6: mesh.intersect(solver='FLOAT', separate_mode='NONE'): a UV sphere cuts the default cube
# (the cut vertices on the triangulation diagonals are dissolved).
# golden: startup=factory compare=objects,context,mesh:Cube tier=T2 tol=1e-4
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.primitive_uv_sphere_add(segments=12, ring_count=6, radius=0.8, location=(0.9, 0.7, 0.45))
bpy.ops.mesh.intersect(solver='FLOAT', separate_mode='NONE')
