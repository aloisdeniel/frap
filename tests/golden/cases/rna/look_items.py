# golden: startup=factory compare=objects tier=T0
# M5+M6 stabilization: `view_settings.look` offers the looks of the current view transform
# (`rna_ColorManagedViewSettings_look_itemf`). Every refused assignment is a TypeError whose
# message lists the current items, compared line by line; the `view_transform` setter's remap
# keeps a compatible look (`High Contrast` on Filmic, then Standard) and moves the others
# (`AgX - Low Contrast` -> `Low Contrast`), checked by assigning a look the remapped view
# refuses. The rendered looks are compared by `render_eevee/scenes/colour_looks_views.py`.
import bpy
bpy.context.scene.view_settings.look = 'High Contrast'
bpy.context.scene.view_settings.look = 'False Color - Punchy'
bpy.context.scene.view_settings.look = 'no such look'
bpy.context.scene.view_settings.look = 'AgX - Low Contrast'
bpy.context.scene.view_settings.view_transform = 'Standard'
bpy.context.scene.view_settings.look = 'AgX - Low Contrast'
bpy.context.scene.view_settings.look = 'Medium Contrast'
bpy.context.scene.view_settings.view_transform = 'Filmic'
bpy.context.scene.view_settings.look = 'AgX - Punchy'
bpy.context.scene.view_settings.look = 'High Contrast'
bpy.context.scene.view_settings.view_transform = 'Filmic Log'
bpy.context.scene.view_settings.view_transform = 'Khronos PBR Neutral'
bpy.context.scene.view_settings.look = 'ACES 1.3 - Reference Gamut Compression'
bpy.context.scene.view_settings.view_transform = 'False Color'
bpy.context.scene.view_settings.look = 'High Contrast'
bpy.context.scene.view_settings.look = 'False Color - Greyscale'
bpy.context.scene.view_settings.view_transform = 'ACES 1.3'
bpy.context.scene.view_settings.look = 'Low Contrast'
bpy.context.scene.view_settings.look = 'ACES 1.3 - Reference Gamut Compression'
bpy.context.scene.view_settings.view_transform = 'ACES 2.0'
bpy.context.scene.view_settings.look = 'ACES 1.3 - Reference Gamut Compression'
bpy.context.scene.view_settings.view_transform = 'Raw'
bpy.context.scene.view_settings.look = 'AgX - Greyscale'
bpy.context.scene.view_settings.look = 'Very Low Contrast'
bpy.context.scene.render.image_settings.color_management = 'OVERRIDE'
bpy.context.scene.render.image_settings.view_settings.view_transform = 'Filmic'
bpy.context.scene.render.image_settings.view_settings.look = 'AgX - Punchy'
bpy.context.scene.render.image_settings.view_settings.look = 'Medium High Contrast'
bpy.data.objects['Cube'].location.x = 1.0
