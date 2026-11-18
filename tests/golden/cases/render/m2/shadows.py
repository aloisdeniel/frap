# golden: startup=factory compare=objects,context tier=T0
# render-golden: image=out/shadows.png min_ssim=0.95
# Render golden (WP 2.3): Workbench shadows (`show_shadows`, `shadow_intensity`,
# `display.light_direction`, `shadow_shift`): stencil shadow volumes of a closed cube, a smooth
# sphere and an open, non-manifold grid (double-counted silhouettes) cast on a ground plane.
import bpy
bpy.ops.mesh.primitive_plane_add(size=8)
bpy.ops.mesh.primitive_uv_sphere_add(radius=0.6, location=(1.6, 0.4, 1.0))
bpy.ops.object.shade_smooth()
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=4, size=1.2, location=(-1.4, -1.0, 1.4), rotation=(0.6, 0.3, 0.0))
bpy.context.scene.render.engine = 'BLENDER_WORKBENCH'
bpy.context.scene.render.resolution_x = 320
bpy.context.scene.render.resolution_y = 180
bpy.context.scene.display.shading.show_shadows = True
bpy.context.scene.display.shading.shadow_intensity = 0.65
bpy.context.scene.display.light_direction = (-0.45, 0.7, 0.55)
bpy.context.scene.display.shadow_shift = 0.2
bpy.context.scene.render.filepath = 'out/shadows'
#@render bpy.ops.render.render(write_still=True)
