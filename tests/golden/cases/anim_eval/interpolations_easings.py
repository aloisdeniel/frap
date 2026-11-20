# WP 6.1: F-curve sampling for the 13 interpolations x 4 easings (AUTO picks each
# equation's default side), keys inserted through the Action data API; the anim: dump
# samples every whole and half frame from 3 before the first key to 3 after the last.
# golden: startup=factory tier=T1 tol=1e-5 compare=anim:Curves
import bpy
bpy.data.actions.new('Curves')
bpy.data.actions['Curves'].slots.new(id_type='OBJECT', name='Cube')
bpy.data.actions['Curves'].layers.new('Layer')
bpy.data.actions['Curves'].layers[0].strips.new(type='KEYFRAME')
bpy.data.actions['Curves'].layers[0].strips[0].channelbag(bpy.data.actions['Curves'].slots[0], ensure=True)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["constant_auto"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points.insert(11, 4.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points.insert(21, -2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points[0].interpolation = 'CONSTANT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points[0].easing = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points[1].interpolation = 'CONSTANT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points[1].easing = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["constant_ease_in"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[1].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[1].keyframe_points.insert(11, 4.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[1].keyframe_points.insert(21, -2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[1].keyframe_points[0].interpolation = 'CONSTANT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[1].keyframe_points[0].easing = 'EASE_IN'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[1].keyframe_points[1].interpolation = 'CONSTANT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[1].keyframe_points[1].easing = 'EASE_IN'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["constant_ease_out"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[2].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[2].keyframe_points.insert(11, 4.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[2].keyframe_points.insert(21, -2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[2].keyframe_points[0].interpolation = 'CONSTANT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[2].keyframe_points[0].easing = 'EASE_OUT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[2].keyframe_points[1].interpolation = 'CONSTANT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[2].keyframe_points[1].easing = 'EASE_OUT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["constant_ease_in_out"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[3].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[3].keyframe_points.insert(11, 4.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[3].keyframe_points.insert(21, -2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[3].keyframe_points[0].interpolation = 'CONSTANT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[3].keyframe_points[0].easing = 'EASE_IN_OUT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[3].keyframe_points[1].interpolation = 'CONSTANT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[3].keyframe_points[1].easing = 'EASE_IN_OUT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["linear_auto"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[4].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[4].keyframe_points.insert(11, 4.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[4].keyframe_points.insert(21, -2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[4].keyframe_points[0].interpolation = 'LINEAR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[4].keyframe_points[0].easing = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[4].keyframe_points[1].interpolation = 'LINEAR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[4].keyframe_points[1].easing = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["linear_ease_in"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[5].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[5].keyframe_points.insert(11, 4.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[5].keyframe_points.insert(21, -2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[5].keyframe_points[0].interpolation = 'LINEAR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[5].keyframe_points[0].easing = 'EASE_IN'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[5].keyframe_points[1].interpolation = 'LINEAR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[5].keyframe_points[1].easing = 'EASE_IN'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["linear_ease_out"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[6].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[6].keyframe_points.insert(11, 4.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[6].keyframe_points.insert(21, -2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[6].keyframe_points[0].interpolation = 'LINEAR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[6].keyframe_points[0].easing = 'EASE_OUT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[6].keyframe_points[1].interpolation = 'LINEAR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[6].keyframe_points[1].easing = 'EASE_OUT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["linear_ease_in_out"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[7].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[7].keyframe_points.insert(11, 4.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[7].keyframe_points.insert(21, -2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[7].keyframe_points[0].interpolation = 'LINEAR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[7].keyframe_points[0].easing = 'EASE_IN_OUT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[7].keyframe_points[1].interpolation = 'LINEAR'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[7].keyframe_points[1].easing = 'EASE_IN_OUT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["bezier_auto"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[8].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[8].keyframe_points.insert(11, 4.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[8].keyframe_points.insert(21, -2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[8].keyframe_points[0].interpolation = 'BEZIER'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[8].keyframe_points[0].easing = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[8].keyframe_points[1].interpolation = 'BEZIER'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[8].keyframe_points[1].easing = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["bezier_ease_in"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[9].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[9].keyframe_points.insert(11, 4.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[9].keyframe_points.insert(21, -2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[9].keyframe_points[0].interpolation = 'BEZIER'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[9].keyframe_points[0].easing = 'EASE_IN'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[9].keyframe_points[1].interpolation = 'BEZIER'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[9].keyframe_points[1].easing = 'EASE_IN'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["bezier_ease_out"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[10].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[10].keyframe_points.insert(11, 4.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[10].keyframe_points.insert(21, -2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[10].keyframe_points[0].interpolation = 'BEZIER'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[10].keyframe_points[0].easing = 'EASE_OUT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[10].keyframe_points[1].interpolation = 'BEZIER'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[10].keyframe_points[1].easing = 'EASE_OUT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["bezier_ease_in_out"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[11].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[11].keyframe_points.insert(11, 4.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[11].keyframe_points.insert(21, -2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[11].keyframe_points[0].interpolation = 'BEZIER'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[11].keyframe_points[0].easing = 'EASE_IN_OUT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[11].keyframe_points[1].interpolation = 'BEZIER'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[11].keyframe_points[1].easing = 'EASE_IN_OUT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["sine_auto"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[12].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[12].keyframe_points.insert(11, 4.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[12].keyframe_points.insert(21, -2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[12].keyframe_points[0].interpolation = 'SINE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[12].keyframe_points[0].easing = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[12].keyframe_points[1].interpolation = 'SINE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[12].keyframe_points[1].easing = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["sine_ease_in"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[13].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[13].keyframe_points.insert(11, 4.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[13].keyframe_points.insert(21, -2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[13].keyframe_points[0].interpolation = 'SINE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[13].keyframe_points[0].easing = 'EASE_IN'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[13].keyframe_points[1].interpolation = 'SINE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[13].keyframe_points[1].easing = 'EASE_IN'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["sine_ease_out"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[14].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[14].keyframe_points.insert(11, 4.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[14].keyframe_points.insert(21, -2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[14].keyframe_points[0].interpolation = 'SINE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[14].keyframe_points[0].easing = 'EASE_OUT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[14].keyframe_points[1].interpolation = 'SINE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[14].keyframe_points[1].easing = 'EASE_OUT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["sine_ease_in_out"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[15].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[15].keyframe_points.insert(11, 4.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[15].keyframe_points.insert(21, -2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[15].keyframe_points[0].interpolation = 'SINE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[15].keyframe_points[0].easing = 'EASE_IN_OUT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[15].keyframe_points[1].interpolation = 'SINE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[15].keyframe_points[1].easing = 'EASE_IN_OUT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["quad_auto"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[16].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[16].keyframe_points.insert(11, 4.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[16].keyframe_points.insert(21, -2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[16].keyframe_points[0].interpolation = 'QUAD'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[16].keyframe_points[0].easing = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[16].keyframe_points[1].interpolation = 'QUAD'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[16].keyframe_points[1].easing = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["quad_ease_in"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[17].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[17].keyframe_points.insert(11, 4.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[17].keyframe_points.insert(21, -2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[17].keyframe_points[0].interpolation = 'QUAD'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[17].keyframe_points[0].easing = 'EASE_IN'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[17].keyframe_points[1].interpolation = 'QUAD'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[17].keyframe_points[1].easing = 'EASE_IN'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["quad_ease_out"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[18].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[18].keyframe_points.insert(11, 4.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[18].keyframe_points.insert(21, -2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[18].keyframe_points[0].interpolation = 'QUAD'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[18].keyframe_points[0].easing = 'EASE_OUT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[18].keyframe_points[1].interpolation = 'QUAD'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[18].keyframe_points[1].easing = 'EASE_OUT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["quad_ease_in_out"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[19].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[19].keyframe_points.insert(11, 4.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[19].keyframe_points.insert(21, -2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[19].keyframe_points[0].interpolation = 'QUAD'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[19].keyframe_points[0].easing = 'EASE_IN_OUT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[19].keyframe_points[1].interpolation = 'QUAD'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[19].keyframe_points[1].easing = 'EASE_IN_OUT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["cubic_auto"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[20].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[20].keyframe_points.insert(11, 4.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[20].keyframe_points.insert(21, -2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[20].keyframe_points[0].interpolation = 'CUBIC'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[20].keyframe_points[0].easing = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[20].keyframe_points[1].interpolation = 'CUBIC'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[20].keyframe_points[1].easing = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["cubic_ease_in"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[21].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[21].keyframe_points.insert(11, 4.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[21].keyframe_points.insert(21, -2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[21].keyframe_points[0].interpolation = 'CUBIC'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[21].keyframe_points[0].easing = 'EASE_IN'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[21].keyframe_points[1].interpolation = 'CUBIC'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[21].keyframe_points[1].easing = 'EASE_IN'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["cubic_ease_out"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[22].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[22].keyframe_points.insert(11, 4.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[22].keyframe_points.insert(21, -2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[22].keyframe_points[0].interpolation = 'CUBIC'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[22].keyframe_points[0].easing = 'EASE_OUT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[22].keyframe_points[1].interpolation = 'CUBIC'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[22].keyframe_points[1].easing = 'EASE_OUT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["cubic_ease_in_out"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[23].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[23].keyframe_points.insert(11, 4.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[23].keyframe_points.insert(21, -2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[23].keyframe_points[0].interpolation = 'CUBIC'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[23].keyframe_points[0].easing = 'EASE_IN_OUT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[23].keyframe_points[1].interpolation = 'CUBIC'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[23].keyframe_points[1].easing = 'EASE_IN_OUT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["quart_auto"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[24].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[24].keyframe_points.insert(11, 4.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[24].keyframe_points.insert(21, -2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[24].keyframe_points[0].interpolation = 'QUART'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[24].keyframe_points[0].easing = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[24].keyframe_points[1].interpolation = 'QUART'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[24].keyframe_points[1].easing = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["quart_ease_in"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[25].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[25].keyframe_points.insert(11, 4.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[25].keyframe_points.insert(21, -2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[25].keyframe_points[0].interpolation = 'QUART'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[25].keyframe_points[0].easing = 'EASE_IN'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[25].keyframe_points[1].interpolation = 'QUART'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[25].keyframe_points[1].easing = 'EASE_IN'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["quart_ease_out"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[26].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[26].keyframe_points.insert(11, 4.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[26].keyframe_points.insert(21, -2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[26].keyframe_points[0].interpolation = 'QUART'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[26].keyframe_points[0].easing = 'EASE_OUT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[26].keyframe_points[1].interpolation = 'QUART'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[26].keyframe_points[1].easing = 'EASE_OUT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["quart_ease_in_out"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[27].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[27].keyframe_points.insert(11, 4.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[27].keyframe_points.insert(21, -2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[27].keyframe_points[0].interpolation = 'QUART'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[27].keyframe_points[0].easing = 'EASE_IN_OUT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[27].keyframe_points[1].interpolation = 'QUART'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[27].keyframe_points[1].easing = 'EASE_IN_OUT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["quint_auto"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[28].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[28].keyframe_points.insert(11, 4.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[28].keyframe_points.insert(21, -2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[28].keyframe_points[0].interpolation = 'QUINT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[28].keyframe_points[0].easing = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[28].keyframe_points[1].interpolation = 'QUINT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[28].keyframe_points[1].easing = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["quint_ease_in"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[29].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[29].keyframe_points.insert(11, 4.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[29].keyframe_points.insert(21, -2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[29].keyframe_points[0].interpolation = 'QUINT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[29].keyframe_points[0].easing = 'EASE_IN'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[29].keyframe_points[1].interpolation = 'QUINT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[29].keyframe_points[1].easing = 'EASE_IN'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["quint_ease_out"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[30].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[30].keyframe_points.insert(11, 4.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[30].keyframe_points.insert(21, -2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[30].keyframe_points[0].interpolation = 'QUINT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[30].keyframe_points[0].easing = 'EASE_OUT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[30].keyframe_points[1].interpolation = 'QUINT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[30].keyframe_points[1].easing = 'EASE_OUT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["quint_ease_in_out"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[31].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[31].keyframe_points.insert(11, 4.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[31].keyframe_points.insert(21, -2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[31].keyframe_points[0].interpolation = 'QUINT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[31].keyframe_points[0].easing = 'EASE_IN_OUT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[31].keyframe_points[1].interpolation = 'QUINT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[31].keyframe_points[1].easing = 'EASE_IN_OUT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["expo_auto"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[32].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[32].keyframe_points.insert(11, 4.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[32].keyframe_points.insert(21, -2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[32].keyframe_points[0].interpolation = 'EXPO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[32].keyframe_points[0].easing = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[32].keyframe_points[1].interpolation = 'EXPO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[32].keyframe_points[1].easing = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["expo_ease_in"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[33].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[33].keyframe_points.insert(11, 4.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[33].keyframe_points.insert(21, -2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[33].keyframe_points[0].interpolation = 'EXPO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[33].keyframe_points[0].easing = 'EASE_IN'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[33].keyframe_points[1].interpolation = 'EXPO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[33].keyframe_points[1].easing = 'EASE_IN'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["expo_ease_out"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[34].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[34].keyframe_points.insert(11, 4.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[34].keyframe_points.insert(21, -2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[34].keyframe_points[0].interpolation = 'EXPO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[34].keyframe_points[0].easing = 'EASE_OUT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[34].keyframe_points[1].interpolation = 'EXPO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[34].keyframe_points[1].easing = 'EASE_OUT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["expo_ease_in_out"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[35].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[35].keyframe_points.insert(11, 4.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[35].keyframe_points.insert(21, -2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[35].keyframe_points[0].interpolation = 'EXPO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[35].keyframe_points[0].easing = 'EASE_IN_OUT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[35].keyframe_points[1].interpolation = 'EXPO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[35].keyframe_points[1].easing = 'EASE_IN_OUT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["circ_auto"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[36].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[36].keyframe_points.insert(11, 4.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[36].keyframe_points.insert(21, -2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[36].keyframe_points[0].interpolation = 'CIRC'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[36].keyframe_points[0].easing = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[36].keyframe_points[1].interpolation = 'CIRC'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[36].keyframe_points[1].easing = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["circ_ease_in"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[37].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[37].keyframe_points.insert(11, 4.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[37].keyframe_points.insert(21, -2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[37].keyframe_points[0].interpolation = 'CIRC'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[37].keyframe_points[0].easing = 'EASE_IN'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[37].keyframe_points[1].interpolation = 'CIRC'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[37].keyframe_points[1].easing = 'EASE_IN'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["circ_ease_out"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[38].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[38].keyframe_points.insert(11, 4.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[38].keyframe_points.insert(21, -2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[38].keyframe_points[0].interpolation = 'CIRC'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[38].keyframe_points[0].easing = 'EASE_OUT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[38].keyframe_points[1].interpolation = 'CIRC'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[38].keyframe_points[1].easing = 'EASE_OUT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["circ_ease_in_out"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[39].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[39].keyframe_points.insert(11, 4.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[39].keyframe_points.insert(21, -2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[39].keyframe_points[0].interpolation = 'CIRC'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[39].keyframe_points[0].easing = 'EASE_IN_OUT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[39].keyframe_points[1].interpolation = 'CIRC'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[39].keyframe_points[1].easing = 'EASE_IN_OUT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["back_auto"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[40].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[40].keyframe_points.insert(11, 4.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[40].keyframe_points.insert(21, -2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[40].keyframe_points[0].interpolation = 'BACK'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[40].keyframe_points[0].easing = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[40].keyframe_points[1].interpolation = 'BACK'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[40].keyframe_points[1].easing = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["back_ease_in"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[41].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[41].keyframe_points.insert(11, 4.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[41].keyframe_points.insert(21, -2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[41].keyframe_points[0].interpolation = 'BACK'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[41].keyframe_points[0].easing = 'EASE_IN'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[41].keyframe_points[1].interpolation = 'BACK'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[41].keyframe_points[1].easing = 'EASE_IN'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["back_ease_out"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[42].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[42].keyframe_points.insert(11, 4.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[42].keyframe_points.insert(21, -2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[42].keyframe_points[0].interpolation = 'BACK'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[42].keyframe_points[0].easing = 'EASE_OUT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[42].keyframe_points[1].interpolation = 'BACK'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[42].keyframe_points[1].easing = 'EASE_OUT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["back_ease_in_out"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[43].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[43].keyframe_points.insert(11, 4.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[43].keyframe_points.insert(21, -2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[43].keyframe_points[0].interpolation = 'BACK'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[43].keyframe_points[0].easing = 'EASE_IN_OUT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[43].keyframe_points[1].interpolation = 'BACK'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[43].keyframe_points[1].easing = 'EASE_IN_OUT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["bounce_auto"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[44].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[44].keyframe_points.insert(11, 4.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[44].keyframe_points.insert(21, -2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[44].keyframe_points[0].interpolation = 'BOUNCE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[44].keyframe_points[0].easing = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[44].keyframe_points[1].interpolation = 'BOUNCE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[44].keyframe_points[1].easing = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["bounce_ease_in"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[45].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[45].keyframe_points.insert(11, 4.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[45].keyframe_points.insert(21, -2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[45].keyframe_points[0].interpolation = 'BOUNCE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[45].keyframe_points[0].easing = 'EASE_IN'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[45].keyframe_points[1].interpolation = 'BOUNCE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[45].keyframe_points[1].easing = 'EASE_IN'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["bounce_ease_out"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[46].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[46].keyframe_points.insert(11, 4.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[46].keyframe_points.insert(21, -2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[46].keyframe_points[0].interpolation = 'BOUNCE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[46].keyframe_points[0].easing = 'EASE_OUT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[46].keyframe_points[1].interpolation = 'BOUNCE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[46].keyframe_points[1].easing = 'EASE_OUT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["bounce_ease_in_out"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[47].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[47].keyframe_points.insert(11, 4.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[47].keyframe_points.insert(21, -2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[47].keyframe_points[0].interpolation = 'BOUNCE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[47].keyframe_points[0].easing = 'EASE_IN_OUT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[47].keyframe_points[1].interpolation = 'BOUNCE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[47].keyframe_points[1].easing = 'EASE_IN_OUT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["elastic_auto"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[48].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[48].keyframe_points.insert(11, 4.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[48].keyframe_points.insert(21, -2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[48].keyframe_points[0].interpolation = 'ELASTIC'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[48].keyframe_points[0].easing = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[48].keyframe_points[1].interpolation = 'ELASTIC'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[48].keyframe_points[1].easing = 'AUTO'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["elastic_ease_in"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[49].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[49].keyframe_points.insert(11, 4.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[49].keyframe_points.insert(21, -2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[49].keyframe_points[0].interpolation = 'ELASTIC'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[49].keyframe_points[0].easing = 'EASE_IN'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[49].keyframe_points[1].interpolation = 'ELASTIC'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[49].keyframe_points[1].easing = 'EASE_IN'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["elastic_ease_out"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[50].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[50].keyframe_points.insert(11, 4.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[50].keyframe_points.insert(21, -2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[50].keyframe_points[0].interpolation = 'ELASTIC'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[50].keyframe_points[0].easing = 'EASE_OUT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[50].keyframe_points[1].interpolation = 'ELASTIC'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[50].keyframe_points[1].easing = 'EASE_OUT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('["elastic_ease_in_out"]', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[51].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[51].keyframe_points.insert(11, 4.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[51].keyframe_points.insert(21, -2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[51].keyframe_points[0].interpolation = 'ELASTIC'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[51].keyframe_points[0].easing = 'EASE_IN_OUT'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[51].keyframe_points[1].interpolation = 'ELASTIC'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[51].keyframe_points[1].easing = 'EASE_IN_OUT'
