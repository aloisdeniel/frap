# WP 2.12: transform.transform in Object Mode with mesh-only modes: SHRINKFATTEN and EDGESLIDE
# fail in their mode init, VERTEX_CREASE changes nothing; without a selected object, CANCELLED.
# golden: startup=factory compare=objects,context tier=T1 tol=1e-5
import bpy
bpy.ops.transform.transform(mode='SHRINKFATTEN', value=(0.5, 0, 0, 0))
bpy.ops.transform.transform(mode='EDGESLIDE', value=(0.5, 0, 0, 0))
bpy.ops.transform.transform(mode='VERTEX_CREASE', value=(0.5, 0, 0, 0))
bpy.ops.object.select_all(action='DESELECT')
bpy.ops.transform.transform(mode='SHRINKFATTEN', value=(0.5, 0, 0, 0))
bpy.ops.transform.transform(mode='EDGESLIDE', value=(0.5, 0, 0, 0))
bpy.ops.transform.transform(mode='VERTEX_CREASE', value=(0.5, 0, 0, 0))
