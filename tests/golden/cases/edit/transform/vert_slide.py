# WP 2.12: transform.vert_slide: towards the neighbor closest to world +X by default, to the
# given direction, use_even (the reference is the vertex closest to the view corner), flipped,
# clamped and unclamped values; UV correction.
# golden: startup=factory compare=objects,context,mesh:Grid tier=T1 tol=1e-5
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=4, size=3, rotation=(0, 0, 0.3))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[12])
bpy.ops.transform.vert_slide(value=0.5)
bpy.ops.transform.vert_slide(value=0.4, direction=(0, 1, 0))
bpy.ops.frap.select_elements(type='VERT', indices=[6, 18, 20])
bpy.ops.transform.vert_slide(value=0.3, direction=(-1, 0, 0))
bpy.ops.transform.vert_slide(value=0.2, use_even=True, direction=(0, -1, 0))
bpy.ops.transform.vert_slide(value=0.2, use_even=True, flipped=True)
bpy.ops.transform.vert_slide(value=1.5)
bpy.ops.transform.vert_slide(value=-0.5, use_clamp=False)
