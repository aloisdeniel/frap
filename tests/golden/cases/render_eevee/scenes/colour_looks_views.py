# golden: startup=factory compare=objects tier=T0
# render-golden: image=out/colour_lv_standard_very_high_contrast.png,out/colour_lv_standard_high_contrast.png,out/colour_lv_standard_medium_high_contrast.png,out/colour_lv_standard_medium_contrast.png,out/colour_lv_standard_medium_low_contrast.png,out/colour_lv_standard_low_contrast.png,out/colour_lv_standard_very_low_contrast.png,out/colour_lv_filmic_very_high_contrast.png,out/colour_lv_filmic_high_contrast.png,out/colour_lv_filmic_medium_high_contrast.png,out/colour_lv_filmic_medium_contrast.png,out/colour_lv_filmic_medium_low_contrast.png,out/colour_lv_filmic_low_contrast.png,out/colour_lv_filmic_very_low_contrast.png,out/colour_lv_filmic_log_medium_high_contrast.png,out/colour_lv_khronos_pbr_neutral_low_contrast.png,out/colour_lv_raw_high_contrast.png,out/colour_lv_false_color_punchy.png,out/colour_lv_false_color_greyscale.png,out/colour_lv_false_color_very_high_contrast.png,out/colour_lv_false_color_high_contrast.png,out/colour_lv_false_color_medium_high_contrast.png,out/colour_lv_false_color_base_contrast.png,out/colour_lv_false_color_medium_low_contrast.png,out/colour_lv_false_color_low_contrast.png,out/colour_lv_false_color_very_low_contrast.png,out/colour_lv_aces_13_reference_gamut_compression.png,out/colour_lv_aces_20_reference_gamut_compression.png,out/colour_lv_remap_agx_low_to_standard.png,out/colour_lv_remap_high_to_false_color.png,out/colour_lv_remap_greyscale_to_filmic.png,out/colour_lv_remap_aces13_to_aces20.png probes=136,16;110,20;89,21;70,21;49,20;23,16;136,45;110,45;89,45;70,45;49,45;23,45;136,73;110,69;89,68;70,68;49,69;23,73
# M5 exit colour golden (M5+M6 stabilization): the looks of the view transforms other than AgX
# (`colour_looks.py` covers AgX) on the swatches of `colour_view_transforms.py`: the generic
# contrast looks on Standard, Filmic, Filmic Log, Khronos PBR Neutral and Raw, every False Color
# look, both ACES gamut compressions, and the look remap of the `view_transform` setter
# (`AgX - Low Contrast` -> Standard renders `Low Contrast`, `AgX - Greyscale` -> Filmic renders
# `None`). Every probe must be within CIE76 ΔE < 1 of Blender's EEVEE. The look items of each
# view transform (TypeErrors listing them) are compared by `rna/look_items.py`.
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
bpy.context.scene.view_settings.view_transform = 'Standard'
bpy.context.scene.view_settings.look = 'Very High Contrast'
bpy.context.scene.render.filepath = 'out/colour_lv_standard_very_high_contrast'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.view_settings.look = 'High Contrast'
bpy.context.scene.render.filepath = 'out/colour_lv_standard_high_contrast'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.view_settings.look = 'Medium High Contrast'
bpy.context.scene.render.filepath = 'out/colour_lv_standard_medium_high_contrast'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.view_settings.look = 'Medium Contrast'
bpy.context.scene.render.filepath = 'out/colour_lv_standard_medium_contrast'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.view_settings.look = 'Medium Low Contrast'
bpy.context.scene.render.filepath = 'out/colour_lv_standard_medium_low_contrast'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.view_settings.look = 'Low Contrast'
bpy.context.scene.render.filepath = 'out/colour_lv_standard_low_contrast'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.view_settings.look = 'Very Low Contrast'
bpy.context.scene.render.filepath = 'out/colour_lv_standard_very_low_contrast'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.view_settings.view_transform = 'Filmic'
bpy.context.scene.view_settings.look = 'Very High Contrast'
bpy.context.scene.render.filepath = 'out/colour_lv_filmic_very_high_contrast'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.view_settings.look = 'High Contrast'
bpy.context.scene.render.filepath = 'out/colour_lv_filmic_high_contrast'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.view_settings.look = 'Medium High Contrast'
bpy.context.scene.render.filepath = 'out/colour_lv_filmic_medium_high_contrast'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.view_settings.look = 'Medium Contrast'
bpy.context.scene.render.filepath = 'out/colour_lv_filmic_medium_contrast'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.view_settings.look = 'Medium Low Contrast'
bpy.context.scene.render.filepath = 'out/colour_lv_filmic_medium_low_contrast'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.view_settings.look = 'Low Contrast'
bpy.context.scene.render.filepath = 'out/colour_lv_filmic_low_contrast'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.view_settings.look = 'Very Low Contrast'
bpy.context.scene.render.filepath = 'out/colour_lv_filmic_very_low_contrast'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.view_settings.view_transform = 'Filmic Log'
bpy.context.scene.view_settings.look = 'Medium High Contrast'
bpy.context.scene.render.filepath = 'out/colour_lv_filmic_log_medium_high_contrast'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.view_settings.view_transform = 'Khronos PBR Neutral'
bpy.context.scene.view_settings.look = 'Low Contrast'
bpy.context.scene.render.filepath = 'out/colour_lv_khronos_pbr_neutral_low_contrast'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.view_settings.view_transform = 'Raw'
bpy.context.scene.view_settings.look = 'High Contrast'
bpy.context.scene.render.filepath = 'out/colour_lv_raw_high_contrast'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.view_settings.view_transform = 'False Color'
bpy.context.scene.view_settings.look = 'False Color - Punchy'
bpy.context.scene.render.filepath = 'out/colour_lv_false_color_punchy'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.view_settings.look = 'False Color - Greyscale'
bpy.context.scene.render.filepath = 'out/colour_lv_false_color_greyscale'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.view_settings.look = 'False Color - Very High Contrast'
bpy.context.scene.render.filepath = 'out/colour_lv_false_color_very_high_contrast'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.view_settings.look = 'False Color - High Contrast'
bpy.context.scene.render.filepath = 'out/colour_lv_false_color_high_contrast'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.view_settings.look = 'False Color - Medium High Contrast'
bpy.context.scene.render.filepath = 'out/colour_lv_false_color_medium_high_contrast'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.view_settings.look = 'False Color - Base Contrast'
bpy.context.scene.render.filepath = 'out/colour_lv_false_color_base_contrast'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.view_settings.look = 'False Color - Medium Low Contrast'
bpy.context.scene.render.filepath = 'out/colour_lv_false_color_medium_low_contrast'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.view_settings.look = 'False Color - Low Contrast'
bpy.context.scene.render.filepath = 'out/colour_lv_false_color_low_contrast'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.view_settings.look = 'False Color - Very Low Contrast'
bpy.context.scene.render.filepath = 'out/colour_lv_false_color_very_low_contrast'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.view_settings.view_transform = 'ACES 1.3'
bpy.context.scene.view_settings.look = 'ACES 1.3 - Reference Gamut Compression'
bpy.context.scene.render.filepath = 'out/colour_lv_aces_13_reference_gamut_compression'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.view_settings.view_transform = 'ACES 2.0'
bpy.context.scene.view_settings.look = 'ACES 2.0 - Reference Gamut Compression'
bpy.context.scene.render.filepath = 'out/colour_lv_aces_20_reference_gamut_compression'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.view_settings.view_transform = 'AgX'
bpy.context.scene.view_settings.look = 'AgX - Low Contrast'
bpy.context.scene.view_settings.view_transform = 'Standard'
bpy.context.scene.render.filepath = 'out/colour_lv_remap_agx_low_to_standard'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.view_settings.view_transform = 'Standard'
bpy.context.scene.view_settings.look = 'High Contrast'
bpy.context.scene.view_settings.view_transform = 'False Color'
bpy.context.scene.render.filepath = 'out/colour_lv_remap_high_to_false_color'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.view_settings.view_transform = 'AgX'
bpy.context.scene.view_settings.look = 'AgX - Greyscale'
bpy.context.scene.view_settings.view_transform = 'Filmic'
bpy.context.scene.render.filepath = 'out/colour_lv_remap_greyscale_to_filmic'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.view_settings.view_transform = 'ACES 1.3'
bpy.context.scene.view_settings.look = 'ACES 1.3 - Reference Gamut Compression'
bpy.context.scene.view_settings.view_transform = 'ACES 2.0'
bpy.context.scene.render.filepath = 'out/colour_lv_remap_aces13_to_aces20'
#@render bpy.ops.render.render(write_still=True)
