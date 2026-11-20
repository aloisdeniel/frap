# golden: startup=factory compare=objects tier=T0
# render-golden: image=out/colour_vt_standard.png,out/colour_vt_aces13.png,out/colour_vt_aces20.png,out/colour_vt_pbr_neutral.png,out/colour_vt_agx.png,out/colour_vt_filmic.png,out/colour_vt_filmic_log.png,out/colour_vt_false_color.png,out/colour_vt_raw.png probes=136,16;110,20;89,21;70,21;49,20;23,16;136,45;110,45;89,45;70,45;49,45;23,45;136,73;110,69;89,68;70,68;49,69;23,73
# M5 exit colour golden (WP 5.6): every view transform of the sRGB display (look None,
# exposure 0, gamma 1) on flat colour swatches. The world is `patches.exr`, an equirectangular
# image of 18 swatches (greys from 0 to 8, saturated and bright colours, written by Blender
# 5.2.4) around a dull background, seen by a 18 mm camera at the origin looking along +Y; each
# probe sits inside one swatch. Every probe must be within CIE76 ΔE < 1 of Blender's EEVEE.
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
bpy.context.scene.render.filepath = 'out/colour_vt_standard'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.view_settings.view_transform = 'ACES 1.3'
bpy.context.scene.render.filepath = 'out/colour_vt_aces13'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.view_settings.view_transform = 'ACES 2.0'
bpy.context.scene.render.filepath = 'out/colour_vt_aces20'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.view_settings.view_transform = 'Khronos PBR Neutral'
bpy.context.scene.render.filepath = 'out/colour_vt_pbr_neutral'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.view_settings.view_transform = 'AgX'
bpy.context.scene.render.filepath = 'out/colour_vt_agx'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.view_settings.view_transform = 'Filmic'
bpy.context.scene.render.filepath = 'out/colour_vt_filmic'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.view_settings.view_transform = 'Filmic Log'
bpy.context.scene.render.filepath = 'out/colour_vt_filmic_log'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.view_settings.view_transform = 'False Color'
bpy.context.scene.render.filepath = 'out/colour_vt_false_color'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.view_settings.view_transform = 'Raw'
bpy.context.scene.render.filepath = 'out/colour_vt_raw'
#@render bpy.ops.render.render(write_still=True)
