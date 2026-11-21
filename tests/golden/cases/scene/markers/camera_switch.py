# WP 7.6: marker camera switching (BKE_scene_camera_switch_update). The scene camera follows the
# markers' cameras at scene.frame_set() (the last marker on or before the frame, else the
# left-most one; cameras with hide_render are skipped) and when a marker's camera is assigned
# (rna_TimelineMarker_camera_update, also for a marker after the current frame); setting
# frame_current, view_layer.update(), evaluated_depsgraph_get(), moving a marker by RNA,
# screen.frame_jump / frame_offset, removing or clearing markers do not switch it. Another
# scene's frame_set switches that scene's camera. Removing a bound camera object clears the
# marker's camera and, when it is the scene camera, the scene camera. After each step a probe
# empty is parented to the scene camera.
# golden: startup=factory tier=T0 compare=objects
import bpy
bpy.ops.object.camera_add(location=(0.0, -6.0, 1.0))
bpy.ops.object.camera_add(location=(6.0, 0.0, 1.0))
bpy.context.scene.camera = bpy.data.objects['Camera']
bpy.context.scene.timeline_markers.new('M1', frame=5)
bpy.context.scene.timeline_markers.new('M2', frame=12)
bpy.context.scene.timeline_markers.new('Plain', frame=8)
bpy.context.scene.timeline_markers[0].camera = bpy.data.objects['Camera.001']
bpy.context.scene.collection.objects.link(bpy.data.objects.new('p01_assign_after_frame', None))
bpy.data.objects['p01_assign_after_frame'].parent = bpy.context.scene.camera
bpy.context.scene.camera = bpy.data.objects['Camera']
bpy.context.scene.timeline_markers[1].camera = bpy.data.objects['Camera.002']
bpy.context.scene.collection.objects.link(bpy.data.objects.new('p02_assign_second', None))
bpy.data.objects['p02_assign_second'].parent = bpy.context.scene.camera
bpy.context.scene.frame_current = 12
bpy.context.view_layer.update()
bpy.context.evaluated_depsgraph_get()
bpy.context.scene.collection.objects.link(bpy.data.objects.new('p03_frame_current', None))
bpy.data.objects['p03_frame_current'].parent = bpy.context.scene.camera
bpy.context.scene.frame_set(12)
bpy.context.scene.collection.objects.link(bpy.data.objects.new('p04_frame_set_12', None))
bpy.data.objects['p04_frame_set_12'].parent = bpy.context.scene.camera
bpy.context.scene.frame_set(11)
bpy.context.scene.collection.objects.link(bpy.data.objects.new('p05_frame_set_11', None))
bpy.data.objects['p05_frame_set_11'].parent = bpy.context.scene.camera
bpy.context.scene.frame_set(-40)
bpy.context.scene.collection.objects.link(bpy.data.objects.new('p06_before_all', None))
bpy.data.objects['p06_before_all'].parent = bpy.context.scene.camera
bpy.context.scene.timeline_markers[0].frame = 30
bpy.context.scene.collection.objects.link(bpy.data.objects.new('p07_marker_moved', None))
bpy.data.objects['p07_marker_moved'].parent = bpy.context.scene.camera
bpy.context.scene.frame_set(20)
bpy.context.scene.collection.objects.link(bpy.data.objects.new('p08_frame_set_20', None))
bpy.data.objects['p08_frame_set_20'].parent = bpy.context.scene.camera
bpy.data.objects['Camera.002'].hide_render = True
bpy.context.scene.frame_set(20)
bpy.context.scene.collection.objects.link(bpy.data.objects.new('p09_hide_render', None))
bpy.data.objects['p09_hide_render'].parent = bpy.context.scene.camera
bpy.data.objects['Camera.002'].hide_render = False
bpy.ops.screen.frame_jump(end=True)
bpy.ops.screen.frame_offset(delta=-200)
bpy.context.scene.collection.objects.link(bpy.data.objects.new('p10_frame_jump', None))
bpy.data.objects['p10_frame_jump'].parent = bpy.context.scene.camera
bpy.context.scene.frame_set(50)
bpy.context.scene.collection.objects.link(bpy.data.objects.new('p11_frame_set_50', None))
bpy.data.objects['p11_frame_set_50'].parent = bpy.context.scene.camera
bpy.context.scene.timeline_markers.remove(bpy.context.scene.timeline_markers[0])
bpy.context.scene.timeline_markers[0].camera = None
bpy.context.scene.collection.objects.link(bpy.data.objects.new('p12_removed_and_unbound', None))
bpy.data.objects['p12_removed_and_unbound'].parent = bpy.context.scene.camera
bpy.context.scene.timeline_markers[1].camera = bpy.data.objects['Cube']
bpy.context.scene.collection.objects.link(bpy.data.objects.new('p13_mesh_as_camera', None))
bpy.data.objects['p13_mesh_as_camera'].parent = bpy.context.scene.camera
bpy.context.scene.timeline_markers.clear()
bpy.context.scene.frame_set(1)
bpy.context.scene.collection.objects.link(bpy.data.objects.new('p14_cleared', None))
bpy.data.objects['p14_cleared'].parent = bpy.context.scene.camera
bpy.data.scenes.new('Other')
bpy.data.scenes['Other'].camera = bpy.data.objects['Camera']
bpy.data.scenes['Other'].timeline_markers.new('O', frame=3)
bpy.data.scenes['Other'].timeline_markers[0].camera = bpy.data.objects['Camera.002']
bpy.data.scenes['Other'].camera = bpy.data.objects['Camera']
bpy.data.scenes['Other'].frame_set(4)
bpy.context.scene.collection.objects.link(bpy.data.objects.new('p15_other_scene', None))
bpy.data.objects['p15_other_scene'].parent = bpy.data.scenes['Other'].camera
bpy.context.scene.collection.objects.link(bpy.data.objects.new('p16_this_scene', None))
bpy.data.objects['p16_this_scene'].parent = bpy.context.scene.camera
bpy.context.scene.timeline_markers.new('K', frame=1)
bpy.context.scene.timeline_markers[0].camera = bpy.data.objects['Camera.001']
bpy.data.objects.remove(bpy.data.objects['Camera.001'])
bpy.context.scene.collection.objects.link(bpy.data.objects.new('p17_camera_removed', None))
bpy.data.objects['p17_camera_removed'].parent = bpy.context.scene.camera
bpy.context.scene.collection.objects.link(bpy.data.objects.new('p18_marker_camera', None))
bpy.data.objects['p18_marker_camera'].parent = bpy.context.scene.timeline_markers[0].camera
