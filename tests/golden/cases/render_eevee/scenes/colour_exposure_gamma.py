# golden: startup=factory compare=objects tier=T0
# render-golden: image=out/colour_eg_standard_exp_gamma.png,out/colour_eg_agx_exp_gamma.png,out/colour_eg_filmic_exp.png,out/colour_eg_pbr_neutral_gamma.png,out/colour_eg_raw_exp.png probes=136,16;110,20;89,21;70,21;49,20;23,16;136,45;110,45;89,45;70,45;49,45;23,45;136,73;110,69;89,68;70,68;49,69;23,73
# M5 exit colour golden (WP 5.6): `view_settings.exposure` and `gamma` with several view
# transforms on the swatches of `colour_view_transforms.py`. Every probe must be within CIE76
# ΔE < 1 of Blender's EEVEE.
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
bpy.context.scene.view_settings.look = 'None'
bpy.context.scene.view_settings.exposure = -1.5
bpy.context.scene.view_settings.gamma = 1.4
bpy.context.scene.render.filepath = 'out/colour_eg_standard_exp_gamma'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.view_settings.view_transform = 'AgX'
bpy.context.scene.view_settings.look = 'None'
bpy.context.scene.view_settings.exposure = 2.0
bpy.context.scene.view_settings.gamma = 0.7
bpy.context.scene.render.filepath = 'out/colour_eg_agx_exp_gamma'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.view_settings.view_transform = 'Filmic'
bpy.context.scene.view_settings.look = 'None'
bpy.context.scene.view_settings.exposure = 1.0
bpy.context.scene.view_settings.gamma = 1.0
bpy.context.scene.render.filepath = 'out/colour_eg_filmic_exp'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.view_settings.view_transform = 'Khronos PBR Neutral'
bpy.context.scene.view_settings.look = 'None'
bpy.context.scene.view_settings.exposure = 0.0
bpy.context.scene.view_settings.gamma = 2.2
bpy.context.scene.render.filepath = 'out/colour_eg_pbr_neutral_gamma'
#@render bpy.ops.render.render(write_still=True)
bpy.context.scene.view_settings.view_transform = 'Raw'
bpy.context.scene.view_settings.look = 'None'
bpy.context.scene.view_settings.exposure = -0.5
bpy.context.scene.view_settings.gamma = 1.0
bpy.context.scene.render.filepath = 'out/colour_eg_raw_exp'
#@render bpy.ops.render.render(write_still=True)
