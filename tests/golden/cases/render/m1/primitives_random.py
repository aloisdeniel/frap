# golden: startup=factory compare=objects,context tier=T0
# render-golden: image=out/primitives_random.png
# Render golden (WP 1.24): several primitives with Workbench RANDOM colours (Blender's
# object-name hash) and the Standard view transform.
import bpy
bpy.data.objects.remove(bpy.data.objects['Cube'])
bpy.ops.mesh.primitive_plane_add(size=8)
bpy.ops.mesh.primitive_uv_sphere_add(radius=0.8, location=(-1.6, 0.2, 0.8))
bpy.ops.mesh.primitive_cone_add(location=(0.8, -1.2, 1.0))
bpy.ops.mesh.primitive_torus_add(location=(1.0, 1.6, 0.3))
bpy.ops.mesh.primitive_cylinder_add(radius=0.4, depth=1.5, location=(-0.6, -2.0, 0.75))
bpy.context.scene.render.engine = 'BLENDER_WORKBENCH'
bpy.context.scene.display.shading.color_type = 'RANDOM'
bpy.context.scene.view_settings.view_transform = 'Standard'
bpy.context.scene.render.resolution_x = 320
bpy.context.scene.render.resolution_y = 180
bpy.context.scene.render.filepath = 'out/primitives_random'
#@render bpy.ops.render.render(write_still=True)
