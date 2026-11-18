# golden: startup=factory compare=objects,context tier=T0
# render-golden: image=out/transparent_material.png min_ssim=0.95
# Render golden (WP 2.3): a material whose viewport alpha is below 1 goes to the transparent
# passes outside X-ray mode, with back-face culling, no specular highlights and
# `render_aa = 'OFF'` (one sample at pixel centres).
import bpy
bpy.ops.mesh.primitive_plane_add(size=6)
bpy.ops.mesh.primitive_uv_sphere_add(radius=0.7, location=(1.4, 0.6, 0.8))
bpy.ops.object.shade_smooth()
bpy.data.materials['Material'].diffuse_color = (0.2, 0.5, 0.9, 0.45)
bpy.context.scene.render.engine = 'BLENDER_WORKBENCH'
bpy.context.scene.render.resolution_x = 320
bpy.context.scene.render.resolution_y = 180
bpy.context.scene.display.shading.show_backface_culling = True
bpy.context.scene.display.shading.show_specular_highlight = False
bpy.context.scene.display.render_aa = 'OFF'
bpy.context.scene.render.filepath = 'out/transparent_material'
#@render bpy.ops.render.render(write_still=True)
