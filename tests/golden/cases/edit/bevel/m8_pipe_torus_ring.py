# WP 8.2: an edge loop around a torus tube, three segments: every vertex of the
# loop is a pipe of valence 4.
# golden: startup=factory compare=objects,context,mesh:Torus tier=T1
import bpy
bpy.ops.mesh.primitive_torus_add(major_segments=12, minor_segments=6)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='EDGE', indices=[0, 2, 4, 6, 8, 10])
bpy.ops.mesh.bevel(offset=0.05, segments=3)
