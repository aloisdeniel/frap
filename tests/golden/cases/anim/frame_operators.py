# WP 6.4: screen.frame_jump, screen.frame_offset and screen.keyframe_jump. Each sets the frame
# (frame_offset clamps at 0) and the new frame's animation is applied by the evaluation after the
# operator; keyframe_jump visits the active object's key times and is cancelled past the last.
# The frames and animated values are moved into object transforms after each step.
# golden: startup=factory tier=T1 tol=1e-5 compare=anim:CubeAction,objects
import bpy
bpy.data.objects['Cube'].keyframe_insert('location', frame=3)
bpy.data.objects['Cube'].location = (6.0, 0.0, 1.0)
bpy.data.objects['Cube'].keyframe_insert('location', frame=12)
bpy.data.objects['Cube'].location.y = 2.0
bpy.data.objects['Cube'].keyframe_insert('location', index=1, frame=8)
bpy.context.scene.frame_end = 40
bpy.ops.screen.frame_jump()
bpy.data.objects['Camera'].location = (bpy.context.scene.frame_current, bpy.data.objects['Cube'].location.x, bpy.data.objects['Cube'].location.y)
bpy.ops.screen.frame_offset(delta=5)
bpy.data.objects['Camera'].rotation_euler = (bpy.context.scene.frame_current, bpy.data.objects['Cube'].location.x, bpy.data.objects['Cube'].location.y)
bpy.ops.screen.keyframe_jump()
bpy.data.objects['Camera'].scale = (bpy.context.scene.frame_current, bpy.data.objects['Cube'].location.x, bpy.data.objects['Cube'].location.y)
bpy.ops.screen.keyframe_jump()
bpy.ops.screen.keyframe_jump()
bpy.data.objects['Light'].location = (bpy.context.scene.frame_current, bpy.data.objects['Cube'].location.x, bpy.data.objects['Cube'].location.y)
bpy.ops.screen.keyframe_jump(next=False)
bpy.ops.screen.keyframe_jump(next=False)
bpy.data.objects['Light'].rotation_euler = (bpy.context.scene.frame_current, bpy.data.objects['Cube'].location.x, bpy.data.objects['Cube'].location.y)
bpy.ops.screen.frame_jump(end=True)
bpy.data.objects['Light'].scale = (bpy.context.scene.frame_current, bpy.data.objects['Cube'].location.x, bpy.data.objects['Cube'].location.y)
bpy.ops.screen.frame_offset(delta=-100)
bpy.data.objects['Cube'].delta_location = (bpy.context.scene.frame_current, bpy.data.objects['Cube'].location.x, bpy.data.objects['Cube'].location.y)
bpy.ops.object.select_all(action='DESELECT')
bpy.ops.screen.keyframe_jump()
bpy.data.objects['Cube'].delta_scale = (bpy.context.scene.frame_current, 1.0, 1.0)
