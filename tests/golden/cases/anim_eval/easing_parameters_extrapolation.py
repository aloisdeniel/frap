# WP 6.1: BACK overshoot and ELASTIC amplitude/period on every easing side, keys at
# fractional frames, LINEAR extrapolation from LINEAR, BEZIER, CONSTANT and SINE ends.
# golden: startup=factory tier=T1 tol=1e-5 compare=anim:Curves
import bpy
bpy.data.actions.new('Curves')
bpy.data.actions['Curves'].slots.new(id_type='OBJECT', name='Cube')
bpy.data.actions['Curves'].layers.new('Layer')
bpy.data.actions['Curves'].layers[0].strips.new(type='KEYFRAME')
bpy.data.actions['Curves'].layers[0].strips[0].channelbag(bpy.data.actions['Curves'].slots[0], ensure=True)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["back_0"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points.insert(9.5, 3.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points[0].interpolation = 'BACK'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points[0].easing = 'EASE_IN'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points[0].back = 0.0
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["back_1"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[1].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[1].keyframe_points.insert(9.5, 3.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[1].keyframe_points[0].interpolation = 'BACK'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[1].keyframe_points[0].easing = 'EASE_OUT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[1].keyframe_points[0].back = 0.0
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["back_2"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[2].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[2].keyframe_points.insert(9.5, 3.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[2].keyframe_points[0].interpolation = 'BACK'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[2].keyframe_points[0].easing = 'EASE_IN_OUT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[2].keyframe_points[0].back = 0.0
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["back_3"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[3].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[3].keyframe_points.insert(9.5, 3.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[3].keyframe_points[0].interpolation = 'BACK'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[3].keyframe_points[0].easing = 'EASE_IN'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[3].keyframe_points[0].back = 1.70158
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["back_4"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[4].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[4].keyframe_points.insert(9.5, 3.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[4].keyframe_points[0].interpolation = 'BACK'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[4].keyframe_points[0].easing = 'EASE_OUT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[4].keyframe_points[0].back = 1.70158
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["back_5"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[5].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[5].keyframe_points.insert(9.5, 3.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[5].keyframe_points[0].interpolation = 'BACK'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[5].keyframe_points[0].easing = 'EASE_IN_OUT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[5].keyframe_points[0].back = 1.70158
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["back_6"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[6].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[6].keyframe_points.insert(9.5, 3.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[6].keyframe_points[0].interpolation = 'BACK'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[6].keyframe_points[0].easing = 'EASE_IN'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[6].keyframe_points[0].back = 3.0
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["back_7"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[7].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[7].keyframe_points.insert(9.5, 3.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[7].keyframe_points[0].interpolation = 'BACK'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[7].keyframe_points[0].easing = 'EASE_OUT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[7].keyframe_points[0].back = 3.0
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["back_8"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[8].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[8].keyframe_points.insert(9.5, 3.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[8].keyframe_points[0].interpolation = 'BACK'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[8].keyframe_points[0].easing = 'EASE_IN_OUT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[8].keyframe_points[0].back = 3.0
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["elastic_9"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[9].keyframe_points.insert(2, 1.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[9].keyframe_points.insert(14, -2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[9].keyframe_points[0].interpolation = 'ELASTIC'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[9].keyframe_points[0].easing = 'EASE_IN'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[9].keyframe_points[0].amplitude = 0.0
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[9].keyframe_points[0].period = 0.0
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["elastic_10"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[10].keyframe_points.insert(2, 1.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[10].keyframe_points.insert(14, -2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[10].keyframe_points[0].interpolation = 'ELASTIC'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[10].keyframe_points[0].easing = 'EASE_OUT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[10].keyframe_points[0].amplitude = 0.0
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[10].keyframe_points[0].period = 0.0
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["elastic_11"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[11].keyframe_points.insert(2, 1.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[11].keyframe_points.insert(14, -2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[11].keyframe_points[0].interpolation = 'ELASTIC'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[11].keyframe_points[0].easing = 'EASE_IN_OUT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[11].keyframe_points[0].amplitude = 0.0
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[11].keyframe_points[0].period = 0.0
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["elastic_12"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[12].keyframe_points.insert(2, 1.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[12].keyframe_points.insert(14, -2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[12].keyframe_points[0].interpolation = 'ELASTIC'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[12].keyframe_points[0].easing = 'EASE_IN'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[12].keyframe_points[0].amplitude = 0.8
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[12].keyframe_points[0].period = 4.1
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["elastic_13"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[13].keyframe_points.insert(2, 1.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[13].keyframe_points.insert(14, -2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[13].keyframe_points[0].interpolation = 'ELASTIC'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[13].keyframe_points[0].easing = 'EASE_OUT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[13].keyframe_points[0].amplitude = 0.8
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[13].keyframe_points[0].period = 4.1
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["elastic_14"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[14].keyframe_points.insert(2, 1.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[14].keyframe_points.insert(14, -2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[14].keyframe_points[0].interpolation = 'ELASTIC'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[14].keyframe_points[0].easing = 'EASE_IN_OUT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[14].keyframe_points[0].amplitude = 0.8
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[14].keyframe_points[0].period = 4.1
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["elastic_15"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[15].keyframe_points.insert(2, 1.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[15].keyframe_points.insert(14, -2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[15].keyframe_points[0].interpolation = 'ELASTIC'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[15].keyframe_points[0].easing = 'EASE_IN'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[15].keyframe_points[0].amplitude = 5.0
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[15].keyframe_points[0].period = 2.0
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["elastic_16"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[16].keyframe_points.insert(2, 1.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[16].keyframe_points.insert(14, -2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[16].keyframe_points[0].interpolation = 'ELASTIC'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[16].keyframe_points[0].easing = 'EASE_OUT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[16].keyframe_points[0].amplitude = 5.0
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[16].keyframe_points[0].period = 2.0
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["elastic_17"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[17].keyframe_points.insert(2, 1.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[17].keyframe_points.insert(14, -2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[17].keyframe_points[0].interpolation = 'ELASTIC'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[17].keyframe_points[0].easing = 'EASE_IN_OUT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[17].keyframe_points[0].amplitude = 5.0
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[17].keyframe_points[0].period = 2.0
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["elastic_18"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[18].keyframe_points.insert(2, 1.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[18].keyframe_points.insert(14, -2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[18].keyframe_points[0].interpolation = 'ELASTIC'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[18].keyframe_points[0].easing = 'EASE_IN'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[18].keyframe_points[0].amplitude = 0.2
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[18].keyframe_points[0].period = 6.0
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["elastic_19"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[19].keyframe_points.insert(2, 1.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[19].keyframe_points.insert(14, -2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[19].keyframe_points[0].interpolation = 'ELASTIC'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[19].keyframe_points[0].easing = 'EASE_OUT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[19].keyframe_points[0].amplitude = 0.2
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[19].keyframe_points[0].period = 6.0
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["elastic_20"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[20].keyframe_points.insert(2, 1.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[20].keyframe_points.insert(14, -2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[20].keyframe_points[0].interpolation = 'ELASTIC'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[20].keyframe_points[0].easing = 'EASE_IN_OUT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[20].keyframe_points[0].amplitude = 0.2
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[20].keyframe_points[0].period = 6.0
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["extrapolate_21"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[21].extrapolation = 'LINEAR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[21].keyframe_points.insert(1.25, 1.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[21].keyframe_points.insert(4.5, 3.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[21].keyframe_points.insert(7.75, 2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[21].keyframe_points[0].interpolation = 'LINEAR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[21].keyframe_points[1].interpolation = 'LINEAR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[21].keyframe_points[2].interpolation = 'LINEAR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["extrapolate_22"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[22].extrapolation = 'LINEAR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[22].keyframe_points.insert(1.25, 1.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[22].keyframe_points.insert(4.5, 3.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[22].keyframe_points.insert(7.75, 2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[22].keyframe_points[0].interpolation = 'BEZIER'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[22].keyframe_points[1].interpolation = 'BEZIER'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[22].keyframe_points[2].interpolation = 'BEZIER'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["extrapolate_23"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[23].extrapolation = 'LINEAR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[23].keyframe_points.insert(1.25, 1.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[23].keyframe_points.insert(4.5, 3.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[23].keyframe_points.insert(7.75, 2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[23].keyframe_points[0].interpolation = 'CONSTANT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[23].keyframe_points[1].interpolation = 'CONSTANT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[23].keyframe_points[2].interpolation = 'CONSTANT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["extrapolate_24"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[24].extrapolation = 'LINEAR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[24].keyframe_points.insert(1.25, 1.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[24].keyframe_points.insert(4.5, 3.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[24].keyframe_points.insert(7.75, 2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[24].keyframe_points[0].interpolation = 'SINE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[24].keyframe_points[1].interpolation = 'SINE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[24].keyframe_points[2].interpolation = 'SINE'
