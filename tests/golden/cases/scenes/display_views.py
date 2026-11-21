# golden: startup=factory tier=T0 compare=objects,context
# M5+M6 stabilization round 9: view_settings.view_transform offers the views of the sibling
# display (`rna_ColorManagedViewSettings_view_transform_itemf`), and a scene display change
# replaces a view the new display does not offer with the display's view of the same family,
# else Standard (`IMB_colormanagement_validate_settings`); the view names are object names.
import bpy
bpy.context.scene.view_settings.view_transform = 'Filmic'
bpy.context.scene.view_settings.look = 'Medium High Contrast'
bpy.context.scene.display_settings.display_device = 'Display P3'
bpy.context.scene.collection.objects.link(bpy.data.objects.new(bpy.context.scene.view_settings.view_transform + ' / ' + bpy.context.scene.view_settings.look, None))
bpy.context.scene.view_settings.view_transform = 'Filmic'
bpy.context.scene.view_settings.view_transform = 'Bogus'
bpy.context.scene.view_settings.view_transform = 'AgX'
bpy.context.scene.view_settings.look = 'AgX - Punchy'
bpy.context.scene.display_settings.display_device = 'Rec.2100-PQ'
bpy.context.scene.collection.objects.link(bpy.data.objects.new(bpy.context.scene.view_settings.view_transform + ' / ' + bpy.context.scene.view_settings.look, None))
bpy.context.scene.view_settings.view_transform = 'ACES 2.0 - HDR 2000 nits'
bpy.context.scene.view_settings.look = 'ACES 2.0 - Reference Gamut Compression'
bpy.context.scene.display_settings.display_device = 'Rec.2100-HLG'
bpy.context.scene.collection.objects.link(bpy.data.objects.new(bpy.context.scene.view_settings.view_transform + ' / ' + bpy.context.scene.view_settings.look, None))
bpy.context.scene.view_settings.view_transform = 'Filmic'
bpy.context.scene.display_settings.display_device = 'sRGB'
bpy.context.scene.collection.objects.link(bpy.data.objects.new(bpy.context.scene.view_settings.view_transform + ' / ' + bpy.context.scene.view_settings.look, None))
bpy.context.scene.view_settings.view_transform = 'Khronos PBR Neutral'
bpy.context.scene.view_settings.exposure = 1.5
bpy.context.scene.display_settings.display_device = 'Rec.2020'
bpy.context.scene.collection.objects.link(bpy.data.objects.new(bpy.context.scene.view_settings.view_transform + ' / ' + bpy.context.scene.view_settings.look, None))
bpy.context.scene.render.image_settings.color_management = 'OVERRIDE'
bpy.context.scene.collection.objects.link(bpy.data.objects.new('override ' + bpy.context.scene.render.image_settings.view_settings.view_transform, None))
bpy.context.scene.render.image_settings.view_settings.view_transform = 'Filmic'
bpy.context.scene.render.image_settings.display_settings.display_device = 'sRGB'
bpy.context.scene.render.image_settings.view_settings.view_transform = 'Filmic'
bpy.context.scene.cursor.location[0] = bpy.context.scene.view_settings.exposure
