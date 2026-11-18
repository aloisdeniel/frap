# WP 2.12: transform.edge_slide of one cube edge: both sides, clamped and unclamped values,
# use_even with and without flipped; UVs follow the slid vertices (correct_uv) and stay with
# correct_uv=False.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1 tol=1e-5
import bpy
bpy.data.objects['Cube'].rotation_euler = (0.2, 0, 0.4)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='EDGE', indices=[0])
bpy.ops.transform.edge_slide(value=0.5)
bpy.ops.transform.edge_slide(value=-0.3)
bpy.ops.transform.edge_slide(value=0.25, use_even=True)
bpy.ops.transform.edge_slide(value=0.25, use_even=True, flipped=True)
bpy.ops.transform.edge_slide(value=1.5, use_clamp=False)
bpy.ops.transform.edge_slide(value=2)
bpy.ops.transform.edge_slide(value=-0.2, correct_uv=False)
bpy.ops.transform.edge_slide(value=0.1, correct_uv=True)
