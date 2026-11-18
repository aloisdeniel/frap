# golden: startup=factory compare=objects,context tier=T0
# render-golden: image=out/outline.png min_ssim=0.95
# Render golden (WP 2.3): Workbench object outline (`show_object_outline`,
# `object_outline_color`) between objects and against the background, with RANDOM colours.
import bpy
bpy.ops.mesh.primitive_plane_add(size=6)
bpy.ops.mesh.primitive_uv_sphere_add(radius=0.6, location=(1.5, 0.5, 0.8))
bpy.ops.object.shade_smooth()
bpy.ops.mesh.primitive_cylinder_add(radius=0.4, depth=1.2, location=(-1.4, -0.6, 0.6))
bpy.context.scene.render.engine = 'BLENDER_WORKBENCH'
bpy.context.scene.render.resolution_x = 320
bpy.context.scene.render.resolution_y = 180
bpy.context.scene.display.shading.color_type = 'RANDOM'
bpy.context.scene.display.shading.show_object_outline = True
bpy.context.scene.display.shading.object_outline_color = (0.9, 0.9, 0.1)
bpy.context.scene.render.filepath = 'out/outline'
#@render bpy.ops.render.render(write_still=True)
