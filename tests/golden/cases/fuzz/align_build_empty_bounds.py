# M8 stabilization round 3 (fuzz seed 0x6aa194f253ee2b65, run 0x5eed8e91): `object.align` of an
# object whose evaluated mesh is empty (BUILD at its first frame). `Object.bound_box` falls back
# to the original data's bounds (`BKE_object_boundbox_eval_cached_get` →
# `BKE_object_boundbox_get`): the cube's -1..1, not zero bounds.
# golden: startup=factory compare=objects tier=T2 tol=0.0001
import bpy
bpy.ops.object.modifier_add(type='BUILD')
bpy.ops.object.align(bb_quality=False, align_mode='OPT_3', relative_to='OPT_3', align_axis={'X'})
bpy.ops.object.align(bb_quality=False, align_mode='OPT_1', relative_to='OPT_1', align_axis={'Y', 'Z'})
