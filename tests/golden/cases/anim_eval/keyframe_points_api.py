# WP 6.1: FCurveKeyframePoints.insert (replace within 0.01 frame, REPLACE / NEEDED / FAST,
# keyframe types, subdivision between manual handles), add / sort / deduplicate /
# handles_recalc / remove / clear, Keyframe.co_ui, FCurve.evaluate / range (moved into the
# Cube's transforms so they are compared) and the errors of remove().
# golden: startup=factory tier=T1 tol=1e-5 compare=anim:Curves,objects
import bpy
bpy.data.actions.new('Curves')
bpy.data.actions['Curves'].slots.new(id_type='OBJECT', name='Cube')
bpy.data.actions['Curves'].layers.new('Layer')
bpy.data.actions['Curves'].layers[0].strips.new(type='KEYFRAME')
bpy.data.actions['Curves'].layers[0].strips[0].channelbag(bpy.data.actions['Curves'].slots[0], ensure=True)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('location', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points.insert(10, 1.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points.insert(2, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points.insert(6, 5.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points.insert(6.004, 2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points.insert(14.5, -1.0, keyframe_type='BREAKDOWN')
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points.insert(30, 9.0, options={'REPLACE'})
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points.insert(10, 1.0, options={'NEEDED'})
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points.insert(8, 4.0, options={'FAST'})
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points[1].co_ui = (3.0, 0.5)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points.remove(bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points[2])
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('location', index=1)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[1].keyframe_points.add(3)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[1].keyframe_points[0].co = (12.0, 2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[1].keyframe_points[1].co = (4.0, -1.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[1].keyframe_points[2].co = (4.004, 6.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[1].keyframe_points.sort()
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[1].keyframe_points.deduplicate()
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[1].keyframe_points.handles_recalc()
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('location', index=2)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[2].keyframe_points.insert(1, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[2].keyframe_points.insert(21, 10.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[2].keyframe_points[0].handle_right_type = 'FREE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[2].keyframe_points[0].handle_left_type = 'FREE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[2].keyframe_points[0].handle_right = (9.0, 6.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[2].keyframe_points[1].handle_left_type = 'FREE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[2].keyframe_points[1].handle_right_type = 'FREE'
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[2].keyframe_points[1].handle_left = (15.0, 1.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[2].keyframe_points.insert(8, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[2].keyframe_points.insert(13.5, 0.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves.new('scale', index=0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[3].keyframe_points.insert(1, 1.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[3].keyframe_points.insert(3, 5.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[3].keyframe_points.insert(4, 5.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[3].keyframe_points.insert(9, -2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[3].keyframe_points.insert(10, 7.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[3].keyframe_points.clear()
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[3].keyframe_points.insert(2, 2.0)
bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[3].keyframe_points.remove(bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points[0])
bpy.data.objects['Cube'].location = (bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[0].evaluate(7.5), bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[1].evaluate(8), bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[0].range()[1])
bpy.data.objects['Cube'].scale = (bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[3].evaluate(0), bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[2].evaluate(11), bpy.data.actions['Curves'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points[2].co[0])
