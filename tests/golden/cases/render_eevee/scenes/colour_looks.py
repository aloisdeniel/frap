# golden: startup=factory compare=objects tier=T0
# render-golden: image=out/colour_look_agx_punchy.png,out/colour_look_agx_greyscale.png,out/colour_look_agx_very_high_contrast.png,out/colour_look_agx_high_contrast.png,out/colour_look_agx_medium_high_contrast.png,out/colour_look_agx_base_contrast.png,out/colour_look_agx_medium_low_contrast.png,out/colour_look_agx_low_contrast.png,out/colour_look_agx_very_low_contrast.png probes=136,16;110,20;89,21;70,21;49,20;23,16;136,45;110,45;89,45;70,45;49,45;23,45;136,73;110,69;89,68;70,68;49,69;23,73
# M5 exit colour golden (WP 5.6): every look of the factory view transform (AgX) on the swatches
# of `colour_view_transforms.py`. Every probe must be within CIE76 ΔE < 1 of Blender's EEVEE.
# The looks of the other view transforms (and the look remap) are not reachable yet: gap in
# docs/deviations.md §2 ("EEVEE looks of view transforms other than AgX", WP 5.3 follow-up).
import bpy
bpy.ops.object.delete()
bpy.data.images.load('patches.exr')
bpy.data.worlds['World'].node_tree.nodes.new('ShaderNodeTexEnvironment')
bpy.data.worlds['World'].node_tree.nodes['Environment Texture'].image = bpy.data.images['patches.exr']
bpy.data.worlds['World'].node_tree.links.new(bpy.data.worlds['World'].node_tree.nodes['Environment Texture'].outputs['Color'], bpy.data.worlds['World'].node_tree.nodes['Background'].inputs['Color'])
bpy.data.worlds['World'].node_tree.nodes['Background'].inputs['Strength'].default_value = 1.0
bpy.data.objects['Camera'].location = (0.0, 0.0, 0.0)
bpy.data.objects['Camera'].rotation_euler = (1.5707963, 0.0, 0.0)
bpy.data.cameras['Camera'].lens = 18.0
bpy.context.scene.render.engine = 'BLENDER_EEVEE'
bpy.context.scene.render.resolution_x = 160
bpy.context.scene.render.resolution_y = 90
bpy.context.scene.eevee.taa_render_samples = 4
bpy.context.scene.view_settings.view_transform = 'AgX'
bpy.context.scene.view_settings.look = 'AgX - Punchy'
bpy.context.scene.render.filepath = 'out/colour_look_agx_punchy'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.view_settings.look = 'AgX - Greyscale'
bpy.context.scene.render.filepath = 'out/colour_look_agx_greyscale'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.view_settings.look = 'AgX - Very High Contrast'
bpy.context.scene.render.filepath = 'out/colour_look_agx_very_high_contrast'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.view_settings.look = 'AgX - High Contrast'
bpy.context.scene.render.filepath = 'out/colour_look_agx_high_contrast'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.view_settings.look = 'AgX - Medium High Contrast'
bpy.context.scene.render.filepath = 'out/colour_look_agx_medium_high_contrast'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.view_settings.look = 'AgX - Base Contrast'
bpy.context.scene.render.filepath = 'out/colour_look_agx_base_contrast'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.view_settings.look = 'AgX - Medium Low Contrast'
bpy.context.scene.render.filepath = 'out/colour_look_agx_medium_low_contrast'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.view_settings.look = 'AgX - Low Contrast'
bpy.context.scene.render.filepath = 'out/colour_look_agx_low_contrast'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.view_settings.look = 'AgX - Very Low Contrast'
bpy.context.scene.render.filepath = 'out/colour_look_agx_very_low_contrast'
#@render bpy.ops.render.render(write_still=True)
