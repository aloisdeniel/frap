# WP 7.6: timeline markers survive a save and an open (names, frames, selection, cameras), and
# wm.open_mainfile checks the file's colour management as Blender's file read does
# (IMB_colormanagement_check_file_config): an output override whose view transform its own
# display lacks ('Filmic' on 'Display P3', kept by the display change) falls back to the view
# colormanage_find_matching_view_name picks ('Standard'; 'AgX' on 'Rec.2100-PQ' becomes
# 'AgX - HDR 1000 nits') and keeps a compatible look; Blender logs `scene output view "Filmic"
# not found, setting to "Standard".` (frap reports it as a WARNING). The marker golden area holds
# the case because it is the WP's. Values are copied into probe names and transforms.
# golden: startup=factory tier=T0 compare=objects
import bpy
bpy.context.scene.timeline_markers.new('Start', frame=1)
bpy.context.scene.timeline_markers.new('Cam', frame=24)
bpy.context.scene.timeline_markers[0].select = False
bpy.context.scene.timeline_markers[1].camera = bpy.data.objects['Camera']
bpy.context.scene.render.image_settings.color_management = 'OVERRIDE'
bpy.context.scene.render.image_settings.view_settings.view_transform = 'Filmic'
bpy.context.scene.render.image_settings.view_settings.look = 'Medium High Contrast'
bpy.context.scene.render.image_settings.display_settings.display_device = 'Display P3'
bpy.ops.wm.save_as_mainfile(filepath='markers.frap')
bpy.context.scene.timeline_markers.clear()
bpy.ops.wm.open_mainfile(filepath='markers.frap')
bpy.context.scene.collection.objects.link(bpy.data.objects.new(bpy.context.scene.render.image_settings.view_settings.view_transform, None))
bpy.context.scene.collection.objects.link(bpy.data.objects.new(bpy.context.scene.render.image_settings.view_settings.look, None))
bpy.context.scene.collection.objects.link(bpy.data.objects.new(bpy.context.scene.view_settings.view_transform, None))
bpy.context.scene.collection.objects.link(bpy.data.objects.new('m0', None))
bpy.data.objects['m0'].location = (bpy.context.scene.timeline_markers[0].frame, bpy.context.scene.timeline_markers[0].select, 0)
bpy.context.scene.collection.objects.link(bpy.data.objects.new('m1', None))
bpy.data.objects['m1'].location = (bpy.context.scene.timeline_markers[1].frame, bpy.context.scene.timeline_markers[1].select, 0)
bpy.data.objects['m1'].parent = bpy.context.scene.timeline_markers[1].camera
bpy.context.scene.timeline_markers[2]
bpy.data.objects['m0'].name = bpy.context.scene.timeline_markers[0].name
bpy.data.objects['m1'].name = bpy.context.scene.timeline_markers[1].name
bpy.context.scene.render.image_settings.view_settings.view_transform = 'AgX'
bpy.context.scene.render.image_settings.view_settings.look = 'AgX - Punchy'
bpy.context.scene.render.image_settings.display_settings.display_device = 'Rec.2100-PQ'
bpy.ops.wm.save_as_mainfile(filepath='markers2.frap')
bpy.ops.wm.open_mainfile(filepath='markers2.frap')
bpy.context.scene.collection.objects.link(bpy.data.objects.new(bpy.context.scene.render.image_settings.view_settings.view_transform, None))
bpy.context.scene.collection.objects.link(bpy.data.objects.new(bpy.context.scene.render.image_settings.view_settings.look, None))
