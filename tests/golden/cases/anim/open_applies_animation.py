# WP 6.4: loading a file evaluates it: right after wm.open_mainfile the originals hold the
# animation of the saved scene's frame, so a manual edit of an animated channel that was saved
# is overwritten (Blender's file read builds and evaluates the depsgraph). Unanimated channels
# keep their saved values.
# golden: startup=factory tier=T1 tol=1e-5 compare=anim:CubeAction,objects
import bpy
bpy.data.objects['Cube'].keyframe_insert('location', index=0, frame=1)
bpy.data.objects['Cube'].location.x = 2.0
bpy.data.objects['Cube'].keyframe_insert('location', index=0, frame=11)
bpy.context.scene.frame_set(6)
bpy.data.objects['Cube'].location.x = 400.0
bpy.data.objects['Cube'].location.y = 3.0
bpy.ops.wm.save_as_mainfile(filepath='anim.frap')
bpy.data.objects['Camera'].location.x = bpy.data.objects['Cube'].location.x
bpy.ops.wm.open_mainfile(filepath='anim.frap')
bpy.data.objects['Camera'].location.y = bpy.data.objects['Cube'].location.x
bpy.data.is_dirty
bpy.data.objects['Cube'].location.x = 400.0
bpy.ops.object.select_all(action='SELECT')
bpy.data.objects['Camera'].location.z = bpy.data.objects['Cube'].location.x
