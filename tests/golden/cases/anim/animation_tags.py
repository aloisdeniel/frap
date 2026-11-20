# WP 6.4, ARCHITECTURE §7.3: an RNA edit whose update callback tags the animation
# (`ID_RECALC_ANIMATION`: a keyframe's `co` or `interpolation`, `FCurve.mute`, the AnimData's
# `action_influence` / `action_slot`, `keyframe_points.insert`, `nla_tracks.new`) makes the next
# evaluation boundary (`view_layer.update()`, an operator) re-apply the animation of the current
# frame to the users of the action, even though time is not dirty; a manual edit of an
# animated channel is then overwritten. Edits without such a tag (`FCurve.select`, `lock`,
# `bpy_struct.keyframe_insert`) keep the manual edit. Each value is copied into an unanimated
# object's transform so it is compared. `frame_set(-3, subframe=0.5)` truncates toward zero
# (frame -2); an unknown operator still applies a pending frame change before it raises.
# golden: startup=factory tier=T1 tol=1e-5 compare=anim:CubeAction,anim:LightAction,objects
import bpy
bpy.data.objects['Cube'].keyframe_insert('location', index=0, frame=1)
bpy.data.objects['Cube'].location.x = 9.0
bpy.data.objects['Cube'].keyframe_insert('location', index=0, frame=10)
bpy.data.objects['Light'].keyframe_insert('location', index=0, frame=1)
bpy.data.objects['Light'].location.x = -3.0
bpy.data.objects['Light'].keyframe_insert('location', index=0, frame=10)
bpy.context.scene.frame_set(4)
bpy.data.objects['Cube'].location.x = 5.0
bpy.data.actions['CubeAction'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points[1].co.y = 20.0
bpy.data.objects['Camera'].location.x = bpy.data.objects['Cube'].location.x
bpy.context.view_layer.update()
bpy.data.objects['Camera'].location.y = bpy.data.objects['Cube'].location.x
bpy.data.objects['Cube'].location.x = 5.0
bpy.data.actions['CubeAction'].layers[0].strips[0].channelbags[0].fcurves[0].select = False
bpy.data.actions['CubeAction'].layers[0].strips[0].channelbags[0].fcurves[0].lock = True
bpy.data.actions['CubeAction'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points[0].select_control_point = False
bpy.context.view_layer.update()
bpy.data.objects['Camera'].location.z = bpy.data.objects['Cube'].location.x
bpy.data.actions['CubeAction'].layers[0].strips[0].channelbags[0].fcurves[0].lock = False
bpy.data.objects['Cube'].location.x = 5.0
bpy.data.actions['CubeAction'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points[0].interpolation = 'LINEAR'
bpy.ops.object.select_all(action='SELECT')
bpy.data.objects['Camera'].rotation_euler.x = bpy.data.objects['Cube'].location.x
bpy.data.objects['Cube'].location.x = 5.0
bpy.data.objects['Cube'].animation_data.action_influence = 0.5
bpy.context.view_layer.update()
bpy.data.objects['Camera'].rotation_euler.y = bpy.data.objects['Cube'].location.x
bpy.data.objects['Cube'].animation_data.action_influence = 1.0
bpy.context.view_layer.update()
bpy.data.objects['Cube'].location.x = 5.0
bpy.data.objects['Cube'].keyframe_insert('location', index=1, frame=20)
bpy.context.view_layer.update()
bpy.data.objects['Camera'].rotation_euler.z = bpy.data.objects['Cube'].location.x
bpy.data.actions['CubeAction'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points.insert(30, 3.0)
bpy.context.view_layer.update()
bpy.data.objects['Camera'].scale.x = bpy.data.objects['Cube'].location.x
bpy.data.objects['Cube'].location.x = 5.0
bpy.data.actions['CubeAction'].layers[0].strips[0].channelbags[0].fcurves[0].mute = True
bpy.context.view_layer.update()
bpy.data.objects['Camera'].scale.y = bpy.data.objects['Cube'].location.x
bpy.data.actions['CubeAction'].layers[0].strips[0].channelbags[0].fcurves[0].mute = False
bpy.context.view_layer.update()
bpy.data.objects['Light'].location.x = 7.0
bpy.data.objects['Light'].animation_data.action_slot = bpy.data.actions['LightAction'].slots[0]
bpy.context.view_layer.update()
bpy.data.objects['Camera'].scale.z = bpy.data.objects['Light'].location.x
bpy.data.objects['Light'].location.x = 7.0
bpy.data.objects['Light'].animation_data.nla_tracks.new()
bpy.context.view_layer.update()
bpy.data.objects['Camera'].delta_location.x = bpy.data.objects['Light'].location.x
bpy.data.objects['Light'].animation_data.nla_tracks.active.name
bpy.data.objects['Light'].animation_data.nla_tracks.new(prev=bpy.data.objects['Light'].animation_data.nla_tracks[0])
bpy.data.objects['Light'].animation_data.nla_tracks.active = bpy.data.objects['Light'].animation_data.nla_tracks[0]
bpy.data.objects['Light'].animation_data.nla_tracks[0].active
bpy.data.objects['Light'].animation_data.nla_tracks[1].active
bpy.data.objects['Light'].animation_data.nla_tracks.active = None
bpy.data.objects['Light'].animation_data.nla_tracks.active
bpy.context.scene.frame_set(-3, subframe=0.5)
bpy.data.objects['Camera'].delta_location.z = bpy.context.scene.frame_current
bpy.context.scene.frame_set(4)
bpy.data.objects['Cube'].location.x = 5.0
bpy.context.scene.frame_current = 7
bpy.ops.object.nonexistent_op()
bpy.data.objects['Camera'].delta_location.y = bpy.data.objects['Cube'].location.x
