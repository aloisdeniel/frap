# WP 2.11: two edges meeting at a cube corner (a weld at that vertex: no vertex mesh),
# three segments.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.frap.select_elements(type='EDGE', indices=[0, 3])
bpy.ops.mesh.bevel(offset=0.25, segments=3)
