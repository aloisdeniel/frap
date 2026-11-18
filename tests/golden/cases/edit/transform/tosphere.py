# WP 2.12: transform.tosphere moves the selection towards the sphere about the pivot whose
# radius is the mean distance; with proportional editing (truncated factor sum) and individual
# origins; Object Mode is not implemented in frap.
# golden: startup=factory compare=objects,context,mesh:Grid tier=T1 tol=1e-5
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=5, y_subdivisions=5, size=2)
bpy.data.objects['Grid'].scale = (1, 1.5, 1)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.transform.tosphere(value=0.5)
bpy.ops.frap.select_elements(type='VERT', indices=[6, 7, 8, 11, 12, 13])
bpy.ops.transform.tosphere(value=1, use_proportional_edit=True, proportional_size=1.2)
bpy.ops.transform.tosphere(value=0.7, center_override=(1, 0, 0))
bpy.ops.transform.tosphere(value=2)
