# WP 2.5: mesh.inset on a 3 x 3 block of grid faces with use_relative_offset and depth: the
# interior vertices take the averaged lengths of the boundary (the fallback pass).
# golden: startup=empty compare=objects,context,mesh:Grid tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=5, y_subdivisions=5, size=2)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[5, 6, 7, 9, 10, 11, 13, 14, 15])
bpy.ops.mesh.inset(thickness=0.1, depth=0.2, use_relative_offset=True)
