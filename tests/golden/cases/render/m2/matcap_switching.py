# golden: startup=factory compare=objects,context tier=T0
# render-golden: image=out/matcap_switching.png min_ssim=0.95
# Render golden (WP 2.3): `studio_light` is one enum over two DNA names routed by `light`
# (`rna_View3DShading_studio_light_get/set`). The matcap set in MATCAP mode survives a studio
# rig set in STUDIO mode: back in MATCAP mode the render uses clay_brown, not rim.sl.
import bpy
bpy.data.objects.remove(bpy.data.objects['Cube'])
bpy.ops.mesh.primitive_monkey_add(location=(0.0, 0.0, 0.9))
bpy.ops.object.shade_smooth()
bpy.context.scene.render.engine = 'BLENDER_WORKBENCH'
bpy.context.scene.render.resolution_x = 320
bpy.context.scene.render.resolution_y = 180
bpy.context.scene.display.shading.light = 'MATCAP'
bpy.context.scene.display.shading.studio_light = 'clay_brown.exr'
bpy.context.scene.display.shading.light = 'STUDIO'
bpy.context.scene.display.shading.studio_light
bpy.context.scene.display.shading.studio_light = 'rim.sl'
bpy.context.scene.display.shading.light = 'MATCAP'
bpy.context.scene.display.shading.studio_light
bpy.context.scene.render.filepath = 'out/matcap_switching'
#@render bpy.ops.render.render(write_still=True)
