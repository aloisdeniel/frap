# golden: startup=factory compare=objects,context tier=T0
# render-golden: image=out/studio_world.png min_ssim=0.95
# Render golden (WP 2.3): a datafile studio rig (`rim.sl`) with world-space lighting rotated by
# `studiolight_rotate_z`, a metallic material, and 16 anti-aliasing samples.
import bpy
bpy.ops.mesh.primitive_plane_add(size=6)
bpy.ops.mesh.primitive_monkey_add(location=(1.4, 0.4, 0.9))
bpy.ops.object.shade_smooth()
bpy.data.materials['Material'].diffuse_color = (0.7, 0.35, 0.2, 1.0)
bpy.data.materials['Material'].metallic = 0.8
bpy.data.materials['Material'].roughness = 0.25
bpy.context.scene.render.engine = 'BLENDER_WORKBENCH'
bpy.context.scene.render.resolution_x = 320
bpy.context.scene.render.resolution_y = 180
bpy.context.scene.display.shading.studio_light = 'rim.sl'
bpy.context.scene.display.shading.use_world_space_lighting = True
bpy.context.scene.display.shading.studiolight_rotate_z = 1.2
bpy.context.scene.display.render_aa = '16'
bpy.context.scene.render.filepath = 'out/studio_world'
#@render bpy.ops.render.render(write_still=True)
