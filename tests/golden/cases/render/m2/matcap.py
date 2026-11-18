# golden: startup=factory compare=objects,context tier=T0
# render-golden: image=out/matcap.png min_ssim=0.95
# Render golden (WP 2.3): Workbench MATCAP lighting with the factory matcap (`basic_bright.exr`,
# diffuse x base colour + specular pass) and OBJECT colours, on a smooth Suzanne, a sphere and a
# flat plane.
import bpy
bpy.data.objects.remove(bpy.data.objects['Cube'])
bpy.ops.mesh.primitive_plane_add(size=6)
bpy.data.objects['Plane'].color = (0.3, 0.45, 0.6, 1.0)
bpy.ops.mesh.primitive_monkey_add(location=(0.0, 0.0, 1.0))
bpy.ops.object.shade_smooth()
bpy.data.objects['Suzanne'].color = (0.9, 0.55, 0.2, 1.0)
bpy.ops.mesh.primitive_uv_sphere_add(radius=0.6, location=(1.8, 0.6, 0.6))
bpy.ops.object.shade_smooth()
bpy.context.scene.render.engine = 'BLENDER_WORKBENCH'
bpy.context.scene.render.resolution_x = 320
bpy.context.scene.render.resolution_y = 180
bpy.context.scene.display.shading.light = 'MATCAP'
bpy.context.scene.display.shading.color_type = 'OBJECT'
bpy.context.scene.render.filepath = 'out/matcap'
#@render bpy.ops.render.render(write_still=True)
