# golden: startup=factory compare=objects,context tier=T0
# render-golden: image=out/wireframe_outline.png min_ssim=0.95
# Render golden (WP 2.3): `shading.type = 'WIREFRAME'` in a final render. Wireframes belong to
# the overlay engine, which F12 renders do not run: the Workbench forces FLAT / OBJECT with an
# X-ray alpha of 0, so surfaces are invisible, but their object ids still drive
# `show_object_outline` (outline colour over the background). A `WIRE` display type object
# renders like the others.
import bpy
bpy.ops.mesh.primitive_plane_add(size=6)
bpy.ops.mesh.primitive_uv_sphere_add(radius=0.6, location=(1.5, 0.5, 0.8))
bpy.data.objects['Sphere'].display_type = 'WIRE'
bpy.context.scene.render.engine = 'BLENDER_WORKBENCH'
bpy.context.scene.render.resolution_x = 320
bpy.context.scene.render.resolution_y = 180
bpy.context.scene.display.shading.type = 'WIREFRAME'
bpy.context.scene.display.shading.show_object_outline = True
bpy.context.scene.display.shading.object_outline_color = (1.0, 0.6, 0.1)
bpy.context.scene.render.filepath = 'out/wireframe_outline'
#@render bpy.ops.render.render(write_still=True)
