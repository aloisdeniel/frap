# golden: startup=factory compare=objects,context tier=T0
# render-golden: image=out/xray.png min_ssim=0.95
# Render golden (WP 2.3): Workbench X-ray (`show_xray`, `xray_alpha`): every surface goes to the
# weighted blended order-independent transparency passes (back faces lit with their unflipped
# normals), over a ground plane, the factory cube and a smooth sphere.
import bpy
bpy.ops.mesh.primitive_plane_add(size=6)
bpy.ops.mesh.primitive_uv_sphere_add(radius=0.6, location=(1.5, 0.5, 0.8))
bpy.ops.object.shade_smooth()
bpy.context.scene.render.engine = 'BLENDER_WORKBENCH'
bpy.context.scene.render.resolution_x = 320
bpy.context.scene.render.resolution_y = 180
bpy.context.scene.display.shading.show_xray = True
bpy.context.scene.display.shading.xray_alpha = 0.35
bpy.context.scene.render.filepath = 'out/xray'
#@render bpy.ops.render.render(write_still=True)
