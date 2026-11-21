# M5+M6 stabilization: switching `window.scene` evaluates the new scene's depsgraph at its own
# frame at once (`ED_scene_change_update`): a FULL_COPY scene left at frame 1 while the source
# scene goes to frame 6 shows its copy's frame-1 values after the switch (and view_layer.update
# keeps them); switching back re-applies the source scene's frame; a frame_set or a
# frame_current change of a scene the window does not show animates nothing. Values are copied
# into object transforms to be compared.
# golden: startup=factory tier=T1 tol=1e-5 compare=objects
import bpy
bpy.data.objects['Cube'].location = (0.0, 0.0, 0.0)
bpy.data.objects['Cube'].keyframe_insert('location', frame=1)
bpy.data.objects['Cube'].location = (4.0, 0.0, 0.0)
bpy.data.objects['Cube'].keyframe_insert('location', frame=11)
bpy.context.scene.frame_set(1)
bpy.ops.scene.new(type='FULL_COPY')
bpy.context.window.scene = bpy.data.scenes['Scene']
bpy.context.scene.frame_set(6)
bpy.data.objects['Camera'].location = bpy.data.objects['Cube'].location
bpy.data.objects['Camera'].rotation_euler = bpy.data.objects['Cube.001'].location
bpy.context.window.scene = bpy.data.scenes['Scene.001']
bpy.data.objects['Camera'].scale = bpy.data.objects['Cube.001'].location
bpy.context.view_layer.update()
bpy.data.objects['Light'].location = bpy.data.objects['Cube.001'].location
bpy.data.objects['Cube'].location = (9.0, 9.0, 9.0)
bpy.data.objects['Cube.001'].location = (9.0, 9.0, 9.0)
bpy.data.scenes['Scene'].frame_set(3)
bpy.data.objects['Light'].rotation_euler = bpy.data.objects['Cube'].location
bpy.data.objects['Light'].scale = bpy.data.objects['Cube.001'].location
bpy.data.scenes['Scene'].frame_current = 4
bpy.context.view_layer.update()
bpy.data.objects['Camera.001'].location = bpy.data.objects['Cube.001'].location
bpy.context.window.scene = bpy.data.scenes['Scene']
bpy.data.objects['Camera.001'].rotation_euler = bpy.data.objects['Cube'].location
bpy.data.objects['Cube'].location = (9.0, 9.0, 9.0)
bpy.context.window.scene = bpy.data.scenes['Scene']
bpy.data.objects['Camera.001'].scale = bpy.data.objects['Cube'].location
bpy.ops.scene.delete()
bpy.data.objects['Light.001'].location = bpy.data.objects['Cube.001'].location
