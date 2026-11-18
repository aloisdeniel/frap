# WP 2.11: vertex bevel in edge select mode (the selection is flushed as in vertex mode),
# two segments, offset type WIDTH.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T2 order=exact
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.frap.select_elements(type='EDGE', indices=[0])
bpy.ops.mesh.bevel(offset=0.2, segments=2, affect='VERTICES', offset_type='WIDTH')
