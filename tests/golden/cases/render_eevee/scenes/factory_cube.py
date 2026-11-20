# golden: startup=factory compare=objects,materials,context tier=T0
# render-golden: image=out/factory_cube.png
# M5 exit scene (WP 5.6): the factory scene as it starts (default cube with the default
# Principled material, the 1000 W point light, the camera, the grey world, AgX) rendered with
# EEVEE at 320 x 180.
import bpy
bpy.context.scene.render.engine = 'BLENDER_EEVEE'
bpy.context.scene.render.resolution_x = 320
bpy.context.scene.render.resolution_y = 180
bpy.context.scene.eevee.taa_render_samples = 16
bpy.context.scene.render.filepath = 'out/factory_cube'
#@render bpy.ops.render.render(write_still=True)
