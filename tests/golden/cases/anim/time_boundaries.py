# WP 6.4, ARCHITECTURE §7.3: animation is applied to the originals only at evaluation
# boundaries. `frame_current = 10` alone leaves the animated location as it is; a manual edit
# followed by keyframe_insert keys the edited x and the pre-boundary y and z (Blender's
# behaviour); view_layer.update(), frame_set() and an operator call then write the animation
# of the frame into the originals (frame_set(3, subframe=0.5) at 3.5). The values are moved into
# object transforms after each step so they are compared. frap stores no sub-frame, so the case
# resets it with frame_set(4) before relying on the frame again (deviations §2).
# golden: startup=factory tier=T1 tol=1e-5 compare=anim:CubeAction,objects
import bpy
bpy.data.objects['Cube'].keyframe_insert('location', frame=1)
bpy.data.objects['Cube'].location = (4.0, 2.0, -2.0)
bpy.data.objects['Cube'].keyframe_insert('location', frame=21)
bpy.context.scene.frame_current = 10
bpy.data.objects['Camera'].location = bpy.data.objects['Cube'].location
bpy.data.objects['Cube'].location.x = 1.0
bpy.data.objects['Cube'].keyframe_insert('location')
bpy.data.objects['Camera'].rotation_euler = bpy.data.objects['Cube'].location
bpy.context.scene.frame_set(5)
bpy.data.objects['Camera'].scale = bpy.data.objects['Cube'].location
bpy.data.objects['Light'].location = bpy.data.objects['Cube'].location
bpy.context.scene.frame_current = 15
bpy.context.view_layer.update()
bpy.data.objects['Light'].rotation_euler = bpy.data.objects['Cube'].location
bpy.context.scene.frame_current = 21
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Light'].scale = bpy.data.objects['Cube'].location
bpy.data.objects['Cube'].location.y = 7.0
bpy.context.scene.frame_set(21)
bpy.data.objects['Cube'].delta_location = bpy.data.objects['Cube'].location
bpy.context.scene.frame_set(3, subframe=0.5)
bpy.data.objects['Cube'].delta_scale = bpy.data.objects['Cube'].location
bpy.context.scene.frame_set(4)
bpy.context.scene.frame_current = 12
bpy.context.view_layer.update()
