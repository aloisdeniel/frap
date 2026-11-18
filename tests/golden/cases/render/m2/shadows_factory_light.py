# golden: startup=factory compare=objects,context tier=T0
# render-golden: image=out/shadows_factory_light.png min_ssim=0.95
# Render golden (WP 2.3): Workbench shadows with the factory light direction, which points
# away from the camera: the shadow volumes are extruded to just inside the far plane (a clipped
# back cap would break the depth-fail count), on a plane that cuts the factory cube and under
# a smooth, non-manifold Suzanne; FLAT lighting with a full shadow intensity shows the
# stencil exactly.
import bpy
bpy.ops.mesh.primitive_plane_add(size=6)
bpy.ops.mesh.primitive_monkey_add(location=(1.5, 0.5, 1.0))
bpy.ops.object.shade_smooth()
bpy.context.scene.render.engine = 'BLENDER_WORKBENCH'
bpy.context.scene.render.resolution_x = 320
bpy.context.scene.render.resolution_y = 180
bpy.context.scene.display.shading.light = 'FLAT'
bpy.context.scene.display.shading.show_shadows = True
bpy.context.scene.display.shading.shadow_intensity = 1.0
bpy.context.scene.render.filepath = 'out/shadows_factory_light'
#@render bpy.ops.render.render(write_still=True)
