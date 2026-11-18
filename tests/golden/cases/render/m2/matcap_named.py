# golden: startup=factory compare=objects,context tier=T0
# render-golden: image=out/matcap_named.png min_ssim=0.95
# Render golden (WP 2.3): a named matcap (`studio_light` in MATCAP mode) with MATERIAL colours
# and back-face culling off, so the inside of the open cylinder shows (matcap back faces keep
# their unflipped normal).
import bpy
bpy.data.objects.remove(bpy.data.objects['Cube'])
bpy.ops.mesh.primitive_monkey_add(location=(-0.6, 0.0, 0.9))
bpy.ops.object.shade_smooth()
bpy.ops.mesh.primitive_cylinder_add(radius=0.6, depth=1.2, end_fill_type='NOTHING', location=(1.4, 0.6, 0.6))
bpy.context.scene.render.engine = 'BLENDER_WORKBENCH'
bpy.context.scene.render.resolution_x = 320
bpy.context.scene.render.resolution_y = 180
bpy.context.scene.display.shading.light = 'MATCAP'
bpy.context.scene.display.shading.studio_light = 'clay_brown.exr'
bpy.context.scene.render.filepath = 'out/matcap_named'
#@render bpy.ops.render.render(write_still=True)
