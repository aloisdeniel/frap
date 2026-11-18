# WP 2.6: mesh.loopcut by edge_index (Blender 5.2.4 runs it headless with a 3D Viewport region
# in the context): the ring of edge 0 cut twice, then a crossing ring cut once; only the new
# loops stay selected (flushed in vertex mode). Out-of-range and default indices cancel.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.loopcut(number_cuts=2, edge_index=0, object_index=0)
bpy.ops.mesh.loopcut(edge_index=20, object_index=0)
bpy.ops.mesh.loopcut(edge_index=99, object_index=0)
bpy.ops.mesh.loopcut(edge_index=1)
