# golden: startup=factory compare=objects,context,mesh:Cube tier=T2 tol=0.0001
# fuzz: seed=0x4a739f6d200771f2 (minimised, M8 stabilization round 1)
import bpy
bpy.ops.object.select_camera(extend=True)
bpy.ops.object.parent_no_inverse_set()
bpy.ops.view3d.snap_selected_to_cursor(use_offset=True)
