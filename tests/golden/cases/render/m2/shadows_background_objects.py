# golden: startup=factory compare=objects,context tier=T0
# render-golden: image=out/shadows_background_objects.png min_ssim=0.97 bg_max_delta_e=2
# Render golden (stabilization, M2): Workbench shadows with no ground plane under a
# non-manifold Suzanne, a torus and a UV sphere whose shadow volumes cross background pixels;
# the background must match Blender's render pixel for pixel (shadows on geometry only).
import bpy
bpy.data.objects['Cube'].hide_render = True
bpy.ops.mesh.primitive_monkey_add(location=(0, 0, 0.5))
bpy.ops.mesh.primitive_torus_add(location=(1.5, 1, 0))
bpy.ops.mesh.primitive_uv_sphere_add(radius=0.5, location=(-1.5, 1, 0.3))
bpy.context.scene.render.engine = 'BLENDER_WORKBENCH'
bpy.context.scene.display.shading.show_shadows = True
bpy.context.scene.render.resolution_x = 480
bpy.context.scene.render.resolution_y = 320
bpy.context.scene.render.filepath = 'out/shadows_background_objects'
#@render bpy.ops.render.render(write_still=True)
