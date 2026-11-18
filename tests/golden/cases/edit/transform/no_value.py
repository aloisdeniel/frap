# WP 2.12: unset values: translate() and rotate() change nothing, resize() collapses the
# selection onto the pivot.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1 tol=1e-5
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[3])
bpy.ops.transform.translate()
bpy.ops.transform.rotate()
bpy.ops.transform.resize()
