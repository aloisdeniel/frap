# golden: startup=factory compare=objects,context tier=T0
# render-golden: image=out/shadows_background_factory.png min_ssim=0.97 bg_max_delta_e=2
# Render golden (stabilization, M2): Workbench shadows with no ground plane. The factory
# light points away from the camera, so the depth-fail volumes of the factory cube reach the
# far plane over background pixels; Blender applies the shadow to geometry only, so the
# background must match its render pixel for pixel (no stray dark pixels under the cube).
import bpy
bpy.context.scene.render.engine = 'BLENDER_WORKBENCH'
bpy.context.scene.display.shading.show_shadows = True
bpy.context.scene.render.resolution_x = 480
bpy.context.scene.render.resolution_y = 320
bpy.context.scene.render.filepath = 'out/shadows_background_factory'
#@render bpy.ops.render.render(write_still=True)
