# WP 6.1: handle recalculation (BKE_fcurve_handles_recalc) for every handle type, with and
# without continuous-acceleration smoothing, constant and linear extrapolation; FREE and
# ALIGNED handles placed by hand, then FCurve.update() / keyframe_points.handles_recalc().
# golden: startup=factory tier=T1 tol=1e-5 compare=anim:Curves
import bpy
bpy.data.actions.new('Curves')
bpy.data.actions['Curves'].slots.new(id_type='OBJECT', name='Cube')
bpy.data.actions['Curves'].layers.new('Layer')
bpy.data.actions['Curves'].layers[0].strips.new(type='KEYFRAME')
bpy.data.actions['Curves'].layers[0].strips[0].channelbag(bpy.data.actions['Curves'].slots[0], ensure=True)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["free_none_constant"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[0].auto_smoothing = 'NONE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[0].extrapolation = 'CONSTANT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points.insert(5, 2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points.insert(9, -1.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points.insert(14, 3.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points.insert(20, 3.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points.insert(26, 0.25)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points[0].handle_left_type = 'FREE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points[0].handle_right_type = 'FREE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points[0].handle_left = (-0.5, -0.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points[0].handle_right = (3.0, 1.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points[1].handle_left_type = 'FREE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points[1].handle_right_type = 'FREE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points[1].handle_left = (3.5, 1.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points[1].handle_right = (7.0, 3.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points[2].handle_left_type = 'FREE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points[2].handle_right_type = 'FREE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points[2].handle_left = (7.5, -1.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points[2].handle_right = (11.0, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points[3].handle_left_type = 'FREE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points[3].handle_right_type = 'FREE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points[3].handle_left = (12.5, 3.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points[3].handle_right = (16.0, 4.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points[4].handle_left_type = 'FREE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points[4].handle_right_type = 'FREE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points[4].handle_left = (18.5, 3.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points[4].handle_right = (22.0, 4.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points[5].handle_left_type = 'FREE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points[5].handle_right_type = 'FREE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points[5].handle_left = (24.5, -0.25)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points[5].handle_right = (28.0, 1.25)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[0].update()
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["aligned_none_constant"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[1].auto_smoothing = 'NONE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[1].extrapolation = 'CONSTANT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[1].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[1].keyframe_points.insert(5, 2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[1].keyframe_points.insert(9, -1.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[1].keyframe_points.insert(14, 3.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[1].keyframe_points.insert(20, 3.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[1].keyframe_points.insert(26, 0.25)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[1].keyframe_points[0].handle_left_type = 'ALIGNED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[1].keyframe_points[0].handle_right_type = 'ALIGNED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[1].keyframe_points[0].handle_left = (-0.5, -0.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[1].keyframe_points[0].handle_right = (3.0, 1.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[1].keyframe_points[1].handle_left_type = 'ALIGNED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[1].keyframe_points[1].handle_right_type = 'ALIGNED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[1].keyframe_points[1].handle_left = (3.5, 1.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[1].keyframe_points[1].handle_right = (7.0, 3.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[1].keyframe_points[2].handle_left_type = 'ALIGNED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[1].keyframe_points[2].handle_right_type = 'ALIGNED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[1].keyframe_points[2].handle_left = (7.5, -1.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[1].keyframe_points[2].handle_right = (11.0, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[1].keyframe_points[3].handle_left_type = 'ALIGNED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[1].keyframe_points[3].handle_right_type = 'ALIGNED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[1].keyframe_points[3].handle_left = (12.5, 3.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[1].keyframe_points[3].handle_right = (16.0, 4.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[1].keyframe_points[4].handle_left_type = 'ALIGNED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[1].keyframe_points[4].handle_right_type = 'ALIGNED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[1].keyframe_points[4].handle_left = (18.5, 3.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[1].keyframe_points[4].handle_right = (22.0, 4.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[1].keyframe_points[5].handle_left_type = 'ALIGNED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[1].keyframe_points[5].handle_right_type = 'ALIGNED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[1].keyframe_points[5].handle_left = (24.5, -0.25)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[1].keyframe_points[5].handle_right = (28.0, 1.25)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[1].update()
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["vector_none_constant"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[2].auto_smoothing = 'NONE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[2].extrapolation = 'CONSTANT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[2].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[2].keyframe_points.insert(5, 2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[2].keyframe_points.insert(9, -1.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[2].keyframe_points.insert(14, 3.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[2].keyframe_points.insert(20, 3.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[2].keyframe_points.insert(26, 0.25)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[2].keyframe_points[0].handle_left_type = 'VECTOR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[2].keyframe_points[0].handle_right_type = 'VECTOR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[2].keyframe_points[1].handle_left_type = 'VECTOR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[2].keyframe_points[1].handle_right_type = 'VECTOR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[2].keyframe_points[2].handle_left_type = 'VECTOR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[2].keyframe_points[2].handle_right_type = 'VECTOR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[2].keyframe_points[3].handle_left_type = 'VECTOR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[2].keyframe_points[3].handle_right_type = 'VECTOR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[2].keyframe_points[4].handle_left_type = 'VECTOR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[2].keyframe_points[4].handle_right_type = 'VECTOR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[2].keyframe_points[5].handle_left_type = 'VECTOR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[2].keyframe_points[5].handle_right_type = 'VECTOR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[2].update()
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["auto_none_constant"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[3].auto_smoothing = 'NONE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[3].extrapolation = 'CONSTANT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[3].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[3].keyframe_points.insert(5, 2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[3].keyframe_points.insert(9, -1.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[3].keyframe_points.insert(14, 3.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[3].keyframe_points.insert(20, 3.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[3].keyframe_points.insert(26, 0.25)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[3].keyframe_points[0].handle_left_type = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[3].keyframe_points[0].handle_right_type = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[3].keyframe_points[1].handle_left_type = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[3].keyframe_points[1].handle_right_type = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[3].keyframe_points[2].handle_left_type = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[3].keyframe_points[2].handle_right_type = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[3].keyframe_points[3].handle_left_type = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[3].keyframe_points[3].handle_right_type = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[3].keyframe_points[4].handle_left_type = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[3].keyframe_points[4].handle_right_type = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[3].keyframe_points[5].handle_left_type = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[3].keyframe_points[5].handle_right_type = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[3].update()
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["auto_clamped_none_constant"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[4].auto_smoothing = 'NONE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[4].extrapolation = 'CONSTANT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[4].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[4].keyframe_points.insert(5, 2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[4].keyframe_points.insert(9, -1.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[4].keyframe_points.insert(14, 3.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[4].keyframe_points.insert(20, 3.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[4].keyframe_points.insert(26, 0.25)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[4].keyframe_points[0].handle_left_type = 'AUTO_CLAMPED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[4].keyframe_points[0].handle_right_type = 'AUTO_CLAMPED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[4].keyframe_points[1].handle_left_type = 'AUTO_CLAMPED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[4].keyframe_points[1].handle_right_type = 'AUTO_CLAMPED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[4].keyframe_points[2].handle_left_type = 'AUTO_CLAMPED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[4].keyframe_points[2].handle_right_type = 'AUTO_CLAMPED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[4].keyframe_points[3].handle_left_type = 'AUTO_CLAMPED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[4].keyframe_points[3].handle_right_type = 'AUTO_CLAMPED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[4].keyframe_points[4].handle_left_type = 'AUTO_CLAMPED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[4].keyframe_points[4].handle_right_type = 'AUTO_CLAMPED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[4].keyframe_points[5].handle_left_type = 'AUTO_CLAMPED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[4].keyframe_points[5].handle_right_type = 'AUTO_CLAMPED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[4].update()
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["mixed_none_constant"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[5].auto_smoothing = 'NONE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[5].extrapolation = 'CONSTANT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[5].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[5].keyframe_points.insert(5, 2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[5].keyframe_points.insert(9, -1.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[5].keyframe_points.insert(14, 3.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[5].keyframe_points.insert(20, 3.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[5].keyframe_points.insert(26, 0.25)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[5].keyframe_points[0].handle_left_type = 'VECTOR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[5].keyframe_points[0].handle_right_type = 'VECTOR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[5].keyframe_points[1].handle_left_type = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[5].keyframe_points[1].handle_right_type = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[5].keyframe_points[2].handle_left_type = 'ALIGNED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[5].keyframe_points[2].handle_right_type = 'ALIGNED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[5].keyframe_points[2].handle_left = (3.0, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[5].keyframe_points[2].handle_right = (7.5, 4.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[5].keyframe_points[4].handle_left_type = 'FREE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[5].keyframe_points[4].handle_right_type = 'VECTOR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[5].keyframe_points[4].handle_left = (18.0, 5.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[5].keyframe_points[5].handle_left_type = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[5].keyframe_points[5].handle_right_type = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[5].keyframe_points.handles_recalc()
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["free_none_linear"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[6].auto_smoothing = 'NONE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[6].extrapolation = 'LINEAR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[6].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[6].keyframe_points.insert(5, 2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[6].keyframe_points.insert(9, -1.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[6].keyframe_points.insert(14, 3.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[6].keyframe_points.insert(20, 3.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[6].keyframe_points.insert(26, 0.25)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[6].keyframe_points[0].handle_left_type = 'FREE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[6].keyframe_points[0].handle_right_type = 'FREE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[6].keyframe_points[0].handle_left = (-0.5, -0.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[6].keyframe_points[0].handle_right = (3.0, 1.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[6].keyframe_points[1].handle_left_type = 'FREE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[6].keyframe_points[1].handle_right_type = 'FREE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[6].keyframe_points[1].handle_left = (3.5, 1.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[6].keyframe_points[1].handle_right = (7.0, 3.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[6].keyframe_points[2].handle_left_type = 'FREE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[6].keyframe_points[2].handle_right_type = 'FREE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[6].keyframe_points[2].handle_left = (7.5, -1.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[6].keyframe_points[2].handle_right = (11.0, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[6].keyframe_points[3].handle_left_type = 'FREE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[6].keyframe_points[3].handle_right_type = 'FREE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[6].keyframe_points[3].handle_left = (12.5, 3.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[6].keyframe_points[3].handle_right = (16.0, 4.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[6].keyframe_points[4].handle_left_type = 'FREE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[6].keyframe_points[4].handle_right_type = 'FREE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[6].keyframe_points[4].handle_left = (18.5, 3.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[6].keyframe_points[4].handle_right = (22.0, 4.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[6].keyframe_points[5].handle_left_type = 'FREE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[6].keyframe_points[5].handle_right_type = 'FREE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[6].keyframe_points[5].handle_left = (24.5, -0.25)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[6].keyframe_points[5].handle_right = (28.0, 1.25)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[6].update()
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["aligned_none_linear"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[7].auto_smoothing = 'NONE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[7].extrapolation = 'LINEAR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[7].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[7].keyframe_points.insert(5, 2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[7].keyframe_points.insert(9, -1.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[7].keyframe_points.insert(14, 3.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[7].keyframe_points.insert(20, 3.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[7].keyframe_points.insert(26, 0.25)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[7].keyframe_points[0].handle_left_type = 'ALIGNED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[7].keyframe_points[0].handle_right_type = 'ALIGNED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[7].keyframe_points[0].handle_left = (-0.5, -0.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[7].keyframe_points[0].handle_right = (3.0, 1.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[7].keyframe_points[1].handle_left_type = 'ALIGNED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[7].keyframe_points[1].handle_right_type = 'ALIGNED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[7].keyframe_points[1].handle_left = (3.5, 1.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[7].keyframe_points[1].handle_right = (7.0, 3.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[7].keyframe_points[2].handle_left_type = 'ALIGNED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[7].keyframe_points[2].handle_right_type = 'ALIGNED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[7].keyframe_points[2].handle_left = (7.5, -1.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[7].keyframe_points[2].handle_right = (11.0, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[7].keyframe_points[3].handle_left_type = 'ALIGNED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[7].keyframe_points[3].handle_right_type = 'ALIGNED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[7].keyframe_points[3].handle_left = (12.5, 3.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[7].keyframe_points[3].handle_right = (16.0, 4.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[7].keyframe_points[4].handle_left_type = 'ALIGNED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[7].keyframe_points[4].handle_right_type = 'ALIGNED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[7].keyframe_points[4].handle_left = (18.5, 3.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[7].keyframe_points[4].handle_right = (22.0, 4.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[7].keyframe_points[5].handle_left_type = 'ALIGNED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[7].keyframe_points[5].handle_right_type = 'ALIGNED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[7].keyframe_points[5].handle_left = (24.5, -0.25)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[7].keyframe_points[5].handle_right = (28.0, 1.25)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[7].update()
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["vector_none_linear"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[8].auto_smoothing = 'NONE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[8].extrapolation = 'LINEAR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[8].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[8].keyframe_points.insert(5, 2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[8].keyframe_points.insert(9, -1.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[8].keyframe_points.insert(14, 3.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[8].keyframe_points.insert(20, 3.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[8].keyframe_points.insert(26, 0.25)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[8].keyframe_points[0].handle_left_type = 'VECTOR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[8].keyframe_points[0].handle_right_type = 'VECTOR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[8].keyframe_points[1].handle_left_type = 'VECTOR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[8].keyframe_points[1].handle_right_type = 'VECTOR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[8].keyframe_points[2].handle_left_type = 'VECTOR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[8].keyframe_points[2].handle_right_type = 'VECTOR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[8].keyframe_points[3].handle_left_type = 'VECTOR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[8].keyframe_points[3].handle_right_type = 'VECTOR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[8].keyframe_points[4].handle_left_type = 'VECTOR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[8].keyframe_points[4].handle_right_type = 'VECTOR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[8].keyframe_points[5].handle_left_type = 'VECTOR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[8].keyframe_points[5].handle_right_type = 'VECTOR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[8].update()
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["auto_none_linear"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[9].auto_smoothing = 'NONE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[9].extrapolation = 'LINEAR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[9].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[9].keyframe_points.insert(5, 2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[9].keyframe_points.insert(9, -1.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[9].keyframe_points.insert(14, 3.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[9].keyframe_points.insert(20, 3.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[9].keyframe_points.insert(26, 0.25)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[9].keyframe_points[0].handle_left_type = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[9].keyframe_points[0].handle_right_type = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[9].keyframe_points[1].handle_left_type = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[9].keyframe_points[1].handle_right_type = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[9].keyframe_points[2].handle_left_type = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[9].keyframe_points[2].handle_right_type = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[9].keyframe_points[3].handle_left_type = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[9].keyframe_points[3].handle_right_type = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[9].keyframe_points[4].handle_left_type = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[9].keyframe_points[4].handle_right_type = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[9].keyframe_points[5].handle_left_type = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[9].keyframe_points[5].handle_right_type = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[9].update()
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["auto_clamped_none_linear"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[10].auto_smoothing = 'NONE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[10].extrapolation = 'LINEAR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[10].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[10].keyframe_points.insert(5, 2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[10].keyframe_points.insert(9, -1.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[10].keyframe_points.insert(14, 3.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[10].keyframe_points.insert(20, 3.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[10].keyframe_points.insert(26, 0.25)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[10].keyframe_points[0].handle_left_type = 'AUTO_CLAMPED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[10].keyframe_points[0].handle_right_type = 'AUTO_CLAMPED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[10].keyframe_points[1].handle_left_type = 'AUTO_CLAMPED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[10].keyframe_points[1].handle_right_type = 'AUTO_CLAMPED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[10].keyframe_points[2].handle_left_type = 'AUTO_CLAMPED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[10].keyframe_points[2].handle_right_type = 'AUTO_CLAMPED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[10].keyframe_points[3].handle_left_type = 'AUTO_CLAMPED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[10].keyframe_points[3].handle_right_type = 'AUTO_CLAMPED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[10].keyframe_points[4].handle_left_type = 'AUTO_CLAMPED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[10].keyframe_points[4].handle_right_type = 'AUTO_CLAMPED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[10].keyframe_points[5].handle_left_type = 'AUTO_CLAMPED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[10].keyframe_points[5].handle_right_type = 'AUTO_CLAMPED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[10].update()
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["mixed_none_linear"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[11].auto_smoothing = 'NONE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[11].extrapolation = 'LINEAR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[11].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[11].keyframe_points.insert(5, 2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[11].keyframe_points.insert(9, -1.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[11].keyframe_points.insert(14, 3.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[11].keyframe_points.insert(20, 3.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[11].keyframe_points.insert(26, 0.25)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[11].keyframe_points[0].handle_left_type = 'VECTOR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[11].keyframe_points[0].handle_right_type = 'VECTOR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[11].keyframe_points[1].handle_left_type = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[11].keyframe_points[1].handle_right_type = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[11].keyframe_points[2].handle_left_type = 'ALIGNED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[11].keyframe_points[2].handle_right_type = 'ALIGNED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[11].keyframe_points[2].handle_left = (3.0, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[11].keyframe_points[2].handle_right = (7.5, 4.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[11].keyframe_points[4].handle_left_type = 'FREE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[11].keyframe_points[4].handle_right_type = 'VECTOR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[11].keyframe_points[4].handle_left = (18.0, 5.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[11].keyframe_points[5].handle_left_type = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[11].keyframe_points[5].handle_right_type = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[11].keyframe_points.handles_recalc()
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["free_cont_accel_constant"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[12].auto_smoothing = 'CONT_ACCEL'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[12].extrapolation = 'CONSTANT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[12].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[12].keyframe_points.insert(5, 2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[12].keyframe_points.insert(9, -1.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[12].keyframe_points.insert(14, 3.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[12].keyframe_points.insert(20, 3.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[12].keyframe_points.insert(26, 0.25)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[12].keyframe_points[0].handle_left_type = 'FREE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[12].keyframe_points[0].handle_right_type = 'FREE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[12].keyframe_points[0].handle_left = (-0.5, -0.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[12].keyframe_points[0].handle_right = (3.0, 1.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[12].keyframe_points[1].handle_left_type = 'FREE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[12].keyframe_points[1].handle_right_type = 'FREE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[12].keyframe_points[1].handle_left = (3.5, 1.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[12].keyframe_points[1].handle_right = (7.0, 3.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[12].keyframe_points[2].handle_left_type = 'FREE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[12].keyframe_points[2].handle_right_type = 'FREE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[12].keyframe_points[2].handle_left = (7.5, -1.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[12].keyframe_points[2].handle_right = (11.0, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[12].keyframe_points[3].handle_left_type = 'FREE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[12].keyframe_points[3].handle_right_type = 'FREE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[12].keyframe_points[3].handle_left = (12.5, 3.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[12].keyframe_points[3].handle_right = (16.0, 4.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[12].keyframe_points[4].handle_left_type = 'FREE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[12].keyframe_points[4].handle_right_type = 'FREE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[12].keyframe_points[4].handle_left = (18.5, 3.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[12].keyframe_points[4].handle_right = (22.0, 4.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[12].keyframe_points[5].handle_left_type = 'FREE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[12].keyframe_points[5].handle_right_type = 'FREE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[12].keyframe_points[5].handle_left = (24.5, -0.25)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[12].keyframe_points[5].handle_right = (28.0, 1.25)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[12].update()
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["aligned_cont_accel_constant"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[13].auto_smoothing = 'CONT_ACCEL'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[13].extrapolation = 'CONSTANT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[13].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[13].keyframe_points.insert(5, 2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[13].keyframe_points.insert(9, -1.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[13].keyframe_points.insert(14, 3.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[13].keyframe_points.insert(20, 3.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[13].keyframe_points.insert(26, 0.25)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[13].keyframe_points[0].handle_left_type = 'ALIGNED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[13].keyframe_points[0].handle_right_type = 'ALIGNED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[13].keyframe_points[0].handle_left = (-0.5, -0.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[13].keyframe_points[0].handle_right = (3.0, 1.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[13].keyframe_points[1].handle_left_type = 'ALIGNED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[13].keyframe_points[1].handle_right_type = 'ALIGNED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[13].keyframe_points[1].handle_left = (3.5, 1.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[13].keyframe_points[1].handle_right = (7.0, 3.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[13].keyframe_points[2].handle_left_type = 'ALIGNED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[13].keyframe_points[2].handle_right_type = 'ALIGNED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[13].keyframe_points[2].handle_left = (7.5, -1.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[13].keyframe_points[2].handle_right = (11.0, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[13].keyframe_points[3].handle_left_type = 'ALIGNED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[13].keyframe_points[3].handle_right_type = 'ALIGNED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[13].keyframe_points[3].handle_left = (12.5, 3.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[13].keyframe_points[3].handle_right = (16.0, 4.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[13].keyframe_points[4].handle_left_type = 'ALIGNED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[13].keyframe_points[4].handle_right_type = 'ALIGNED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[13].keyframe_points[4].handle_left = (18.5, 3.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[13].keyframe_points[4].handle_right = (22.0, 4.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[13].keyframe_points[5].handle_left_type = 'ALIGNED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[13].keyframe_points[5].handle_right_type = 'ALIGNED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[13].keyframe_points[5].handle_left = (24.5, -0.25)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[13].keyframe_points[5].handle_right = (28.0, 1.25)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[13].update()
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["vector_cont_accel_constant"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[14].auto_smoothing = 'CONT_ACCEL'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[14].extrapolation = 'CONSTANT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[14].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[14].keyframe_points.insert(5, 2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[14].keyframe_points.insert(9, -1.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[14].keyframe_points.insert(14, 3.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[14].keyframe_points.insert(20, 3.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[14].keyframe_points.insert(26, 0.25)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[14].keyframe_points[0].handle_left_type = 'VECTOR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[14].keyframe_points[0].handle_right_type = 'VECTOR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[14].keyframe_points[1].handle_left_type = 'VECTOR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[14].keyframe_points[1].handle_right_type = 'VECTOR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[14].keyframe_points[2].handle_left_type = 'VECTOR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[14].keyframe_points[2].handle_right_type = 'VECTOR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[14].keyframe_points[3].handle_left_type = 'VECTOR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[14].keyframe_points[3].handle_right_type = 'VECTOR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[14].keyframe_points[4].handle_left_type = 'VECTOR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[14].keyframe_points[4].handle_right_type = 'VECTOR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[14].keyframe_points[5].handle_left_type = 'VECTOR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[14].keyframe_points[5].handle_right_type = 'VECTOR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[14].update()
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["auto_cont_accel_constant"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[15].auto_smoothing = 'CONT_ACCEL'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[15].extrapolation = 'CONSTANT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[15].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[15].keyframe_points.insert(5, 2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[15].keyframe_points.insert(9, -1.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[15].keyframe_points.insert(14, 3.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[15].keyframe_points.insert(20, 3.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[15].keyframe_points.insert(26, 0.25)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[15].keyframe_points[0].handle_left_type = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[15].keyframe_points[0].handle_right_type = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[15].keyframe_points[1].handle_left_type = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[15].keyframe_points[1].handle_right_type = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[15].keyframe_points[2].handle_left_type = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[15].keyframe_points[2].handle_right_type = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[15].keyframe_points[3].handle_left_type = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[15].keyframe_points[3].handle_right_type = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[15].keyframe_points[4].handle_left_type = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[15].keyframe_points[4].handle_right_type = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[15].keyframe_points[5].handle_left_type = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[15].keyframe_points[5].handle_right_type = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[15].update()
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["auto_clamped_cont_accel_constant"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[16].auto_smoothing = 'CONT_ACCEL'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[16].extrapolation = 'CONSTANT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[16].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[16].keyframe_points.insert(5, 2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[16].keyframe_points.insert(9, -1.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[16].keyframe_points.insert(14, 3.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[16].keyframe_points.insert(20, 3.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[16].keyframe_points.insert(26, 0.25)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[16].keyframe_points[0].handle_left_type = 'AUTO_CLAMPED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[16].keyframe_points[0].handle_right_type = 'AUTO_CLAMPED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[16].keyframe_points[1].handle_left_type = 'AUTO_CLAMPED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[16].keyframe_points[1].handle_right_type = 'AUTO_CLAMPED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[16].keyframe_points[2].handle_left_type = 'AUTO_CLAMPED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[16].keyframe_points[2].handle_right_type = 'AUTO_CLAMPED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[16].keyframe_points[3].handle_left_type = 'AUTO_CLAMPED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[16].keyframe_points[3].handle_right_type = 'AUTO_CLAMPED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[16].keyframe_points[4].handle_left_type = 'AUTO_CLAMPED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[16].keyframe_points[4].handle_right_type = 'AUTO_CLAMPED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[16].keyframe_points[5].handle_left_type = 'AUTO_CLAMPED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[16].keyframe_points[5].handle_right_type = 'AUTO_CLAMPED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[16].update()
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["mixed_cont_accel_constant"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[17].auto_smoothing = 'CONT_ACCEL'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[17].extrapolation = 'CONSTANT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[17].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[17].keyframe_points.insert(5, 2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[17].keyframe_points.insert(9, -1.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[17].keyframe_points.insert(14, 3.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[17].keyframe_points.insert(20, 3.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[17].keyframe_points.insert(26, 0.25)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[17].keyframe_points[0].handle_left_type = 'VECTOR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[17].keyframe_points[0].handle_right_type = 'VECTOR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[17].keyframe_points[1].handle_left_type = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[17].keyframe_points[1].handle_right_type = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[17].keyframe_points[2].handle_left_type = 'ALIGNED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[17].keyframe_points[2].handle_right_type = 'ALIGNED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[17].keyframe_points[2].handle_left = (3.0, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[17].keyframe_points[2].handle_right = (7.5, 4.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[17].keyframe_points[4].handle_left_type = 'FREE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[17].keyframe_points[4].handle_right_type = 'VECTOR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[17].keyframe_points[4].handle_left = (18.0, 5.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[17].keyframe_points[5].handle_left_type = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[17].keyframe_points[5].handle_right_type = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[17].keyframe_points.handles_recalc()
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["free_cont_accel_linear"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[18].auto_smoothing = 'CONT_ACCEL'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[18].extrapolation = 'LINEAR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[18].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[18].keyframe_points.insert(5, 2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[18].keyframe_points.insert(9, -1.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[18].keyframe_points.insert(14, 3.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[18].keyframe_points.insert(20, 3.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[18].keyframe_points.insert(26, 0.25)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[18].keyframe_points[0].handle_left_type = 'FREE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[18].keyframe_points[0].handle_right_type = 'FREE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[18].keyframe_points[0].handle_left = (-0.5, -0.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[18].keyframe_points[0].handle_right = (3.0, 1.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[18].keyframe_points[1].handle_left_type = 'FREE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[18].keyframe_points[1].handle_right_type = 'FREE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[18].keyframe_points[1].handle_left = (3.5, 1.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[18].keyframe_points[1].handle_right = (7.0, 3.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[18].keyframe_points[2].handle_left_type = 'FREE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[18].keyframe_points[2].handle_right_type = 'FREE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[18].keyframe_points[2].handle_left = (7.5, -1.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[18].keyframe_points[2].handle_right = (11.0, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[18].keyframe_points[3].handle_left_type = 'FREE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[18].keyframe_points[3].handle_right_type = 'FREE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[18].keyframe_points[3].handle_left = (12.5, 3.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[18].keyframe_points[3].handle_right = (16.0, 4.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[18].keyframe_points[4].handle_left_type = 'FREE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[18].keyframe_points[4].handle_right_type = 'FREE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[18].keyframe_points[4].handle_left = (18.5, 3.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[18].keyframe_points[4].handle_right = (22.0, 4.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[18].keyframe_points[5].handle_left_type = 'FREE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[18].keyframe_points[5].handle_right_type = 'FREE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[18].keyframe_points[5].handle_left = (24.5, -0.25)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[18].keyframe_points[5].handle_right = (28.0, 1.25)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[18].update()
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["aligned_cont_accel_linear"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[19].auto_smoothing = 'CONT_ACCEL'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[19].extrapolation = 'LINEAR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[19].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[19].keyframe_points.insert(5, 2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[19].keyframe_points.insert(9, -1.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[19].keyframe_points.insert(14, 3.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[19].keyframe_points.insert(20, 3.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[19].keyframe_points.insert(26, 0.25)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[19].keyframe_points[0].handle_left_type = 'ALIGNED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[19].keyframe_points[0].handle_right_type = 'ALIGNED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[19].keyframe_points[0].handle_left = (-0.5, -0.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[19].keyframe_points[0].handle_right = (3.0, 1.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[19].keyframe_points[1].handle_left_type = 'ALIGNED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[19].keyframe_points[1].handle_right_type = 'ALIGNED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[19].keyframe_points[1].handle_left = (3.5, 1.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[19].keyframe_points[1].handle_right = (7.0, 3.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[19].keyframe_points[2].handle_left_type = 'ALIGNED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[19].keyframe_points[2].handle_right_type = 'ALIGNED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[19].keyframe_points[2].handle_left = (7.5, -1.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[19].keyframe_points[2].handle_right = (11.0, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[19].keyframe_points[3].handle_left_type = 'ALIGNED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[19].keyframe_points[3].handle_right_type = 'ALIGNED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[19].keyframe_points[3].handle_left = (12.5, 3.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[19].keyframe_points[3].handle_right = (16.0, 4.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[19].keyframe_points[4].handle_left_type = 'ALIGNED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[19].keyframe_points[4].handle_right_type = 'ALIGNED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[19].keyframe_points[4].handle_left = (18.5, 3.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[19].keyframe_points[4].handle_right = (22.0, 4.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[19].keyframe_points[5].handle_left_type = 'ALIGNED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[19].keyframe_points[5].handle_right_type = 'ALIGNED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[19].keyframe_points[5].handle_left = (24.5, -0.25)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[19].keyframe_points[5].handle_right = (28.0, 1.25)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[19].update()
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["vector_cont_accel_linear"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[20].auto_smoothing = 'CONT_ACCEL'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[20].extrapolation = 'LINEAR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[20].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[20].keyframe_points.insert(5, 2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[20].keyframe_points.insert(9, -1.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[20].keyframe_points.insert(14, 3.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[20].keyframe_points.insert(20, 3.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[20].keyframe_points.insert(26, 0.25)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[20].keyframe_points[0].handle_left_type = 'VECTOR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[20].keyframe_points[0].handle_right_type = 'VECTOR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[20].keyframe_points[1].handle_left_type = 'VECTOR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[20].keyframe_points[1].handle_right_type = 'VECTOR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[20].keyframe_points[2].handle_left_type = 'VECTOR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[20].keyframe_points[2].handle_right_type = 'VECTOR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[20].keyframe_points[3].handle_left_type = 'VECTOR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[20].keyframe_points[3].handle_right_type = 'VECTOR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[20].keyframe_points[4].handle_left_type = 'VECTOR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[20].keyframe_points[4].handle_right_type = 'VECTOR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[20].keyframe_points[5].handle_left_type = 'VECTOR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[20].keyframe_points[5].handle_right_type = 'VECTOR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[20].update()
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["auto_cont_accel_linear"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[21].auto_smoothing = 'CONT_ACCEL'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[21].extrapolation = 'LINEAR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[21].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[21].keyframe_points.insert(5, 2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[21].keyframe_points.insert(9, -1.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[21].keyframe_points.insert(14, 3.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[21].keyframe_points.insert(20, 3.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[21].keyframe_points.insert(26, 0.25)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[21].keyframe_points[0].handle_left_type = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[21].keyframe_points[0].handle_right_type = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[21].keyframe_points[1].handle_left_type = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[21].keyframe_points[1].handle_right_type = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[21].keyframe_points[2].handle_left_type = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[21].keyframe_points[2].handle_right_type = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[21].keyframe_points[3].handle_left_type = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[21].keyframe_points[3].handle_right_type = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[21].keyframe_points[4].handle_left_type = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[21].keyframe_points[4].handle_right_type = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[21].keyframe_points[5].handle_left_type = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[21].keyframe_points[5].handle_right_type = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[21].update()
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["auto_clamped_cont_accel_linear"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[22].auto_smoothing = 'CONT_ACCEL'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[22].extrapolation = 'LINEAR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[22].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[22].keyframe_points.insert(5, 2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[22].keyframe_points.insert(9, -1.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[22].keyframe_points.insert(14, 3.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[22].keyframe_points.insert(20, 3.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[22].keyframe_points.insert(26, 0.25)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[22].keyframe_points[0].handle_left_type = 'AUTO_CLAMPED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[22].keyframe_points[0].handle_right_type = 'AUTO_CLAMPED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[22].keyframe_points[1].handle_left_type = 'AUTO_CLAMPED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[22].keyframe_points[1].handle_right_type = 'AUTO_CLAMPED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[22].keyframe_points[2].handle_left_type = 'AUTO_CLAMPED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[22].keyframe_points[2].handle_right_type = 'AUTO_CLAMPED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[22].keyframe_points[3].handle_left_type = 'AUTO_CLAMPED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[22].keyframe_points[3].handle_right_type = 'AUTO_CLAMPED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[22].keyframe_points[4].handle_left_type = 'AUTO_CLAMPED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[22].keyframe_points[4].handle_right_type = 'AUTO_CLAMPED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[22].keyframe_points[5].handle_left_type = 'AUTO_CLAMPED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[22].keyframe_points[5].handle_right_type = 'AUTO_CLAMPED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[22].update()
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["mixed_cont_accel_linear"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[23].auto_smoothing = 'CONT_ACCEL'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[23].extrapolation = 'LINEAR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[23].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[23].keyframe_points.insert(5, 2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[23].keyframe_points.insert(9, -1.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[23].keyframe_points.insert(14, 3.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[23].keyframe_points.insert(20, 3.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[23].keyframe_points.insert(26, 0.25)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[23].keyframe_points[0].handle_left_type = 'VECTOR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[23].keyframe_points[0].handle_right_type = 'VECTOR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[23].keyframe_points[1].handle_left_type = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[23].keyframe_points[1].handle_right_type = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[23].keyframe_points[2].handle_left_type = 'ALIGNED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[23].keyframe_points[2].handle_right_type = 'ALIGNED'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[23].keyframe_points[2].handle_left = (3.0, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[23].keyframe_points[2].handle_right = (7.5, 4.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[23].keyframe_points[4].handle_left_type = 'FREE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[23].keyframe_points[4].handle_right_type = 'VECTOR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[23].keyframe_points[4].handle_left = (18.0, 5.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[23].keyframe_points[5].handle_left_type = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[23].keyframe_points[5].handle_right_type = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[23].keyframe_points.handles_recalc()
