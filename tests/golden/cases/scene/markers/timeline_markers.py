# WP 7.6: Scene.timeline_markers (TimelineMarkers.new/remove/clear, TimelineMarker.name/frame/
# select/camera). new() appends a selected marker named as given (names repeat; 63 bytes at
# most), frame within +-1048574; name/frame/select are plain writes (any int frame); camera takes
# any object and must be an Object; remove() of another scene's marker is RuntimeError; keyed
# reads find the first marker of that name. Each marker's state is copied into a probe empty
# (location = frame, select; parent = camera), renamed after the marker at the end.
# golden: startup=factory tier=T0 compare=objects
import bpy
bpy.context.scene.timeline_markers.new('Intro', frame=5)
bpy.context.scene.timeline_markers.new('B', frame=12)
bpy.context.scene.timeline_markers.new('B')
bpy.context.scene.timeline_markers.new('Far', frame=2000000)
bpy.context.scene.timeline_markers.new('L' * 80, frame=-3)
bpy.context.scene.timeline_markers.new()
bpy.context.scene.timeline_markers['B'].frame = 30
bpy.context.scene.timeline_markers[2].select = False
bpy.context.scene.timeline_markers[0].name = 'Opening'
bpy.context.scene.timeline_markers[3].frame = 2000000000
bpy.context.scene.timeline_markers[1].camera = bpy.data.objects['Cube']
bpy.context.scene.timeline_markers[1].camera = bpy.data.materials['Material']
bpy.context.scene.timeline_markers['Nope']
bpy.context.scene.timeline_markers[9]
bpy.data.scenes.new('Other')
bpy.data.scenes['Other'].timeline_markers.new('O', frame=7)
bpy.context.scene.timeline_markers.remove(bpy.data.scenes['Other'].timeline_markers[0])
bpy.data.scenes['Other'].timeline_markers.remove(bpy.data.scenes['Other'].timeline_markers[0])
bpy.data.scenes['Other'].timeline_markers[0]
bpy.data.scenes['Other'].timeline_markers.new('O1', frame=1)
bpy.data.scenes['Other'].timeline_markers.new('O2', frame=2)
bpy.data.scenes['Other'].timeline_markers.clear()
bpy.data.scenes['Other'].timeline_markers[0]
bpy.context.scene.timeline_markers.remove(bpy.context.scene.timeline_markers[4])
bpy.context.scene.timeline_markers[4]
bpy.context.scene.collection.objects.link(bpy.data.objects.new('m0', None))
bpy.data.objects['m0'].location = (bpy.context.scene.timeline_markers[0].frame, bpy.context.scene.timeline_markers[0].select, 0)
bpy.data.objects['m0'].parent = bpy.context.scene.timeline_markers[0].camera
bpy.context.scene.collection.objects.link(bpy.data.objects.new('m1', None))
bpy.data.objects['m1'].location = (bpy.context.scene.timeline_markers[1].frame, bpy.context.scene.timeline_markers[1].select, 0)
bpy.data.objects['m1'].parent = bpy.context.scene.timeline_markers[1].camera
bpy.context.scene.collection.objects.link(bpy.data.objects.new('m2', None))
bpy.data.objects['m2'].location = (bpy.context.scene.timeline_markers[2].frame, bpy.context.scene.timeline_markers[2].select, 0)
bpy.context.scene.collection.objects.link(bpy.data.objects.new('m3', None))
bpy.data.objects['m3'].location = (bpy.context.scene.timeline_markers[3].frame / 1000000, bpy.context.scene.timeline_markers[3].select, 0)
bpy.data.objects['m0'].name = bpy.context.scene.timeline_markers[0].name
bpy.data.objects['m1'].name = bpy.context.scene.timeline_markers[1].name
bpy.data.objects['m2'].name = bpy.context.scene.timeline_markers[2].name
bpy.data.objects['m3'].name = bpy.context.scene.timeline_markers[3].name
