# golden: startup=factory compare=objects,context tier=T0
# render-golden: image=out/Scene_320x180.png
# Render golden (WP 1.24): Workbench FLAT lighting with OBJECT colours (Object.color),
# resolution_percentage, an RGB PNG, and a path template (`{scene_name}`, `{resolution_x}`
# after the percentage) in the output path.
import bpy
bpy.data.objects.remove(bpy.data.objects['Cube'])
bpy.ops.mesh.primitive_monkey_add(location=(0.0, 0.0, 1.0))
bpy.data.objects['Suzanne'].color = (0.9, 0.5, 0.1, 1.0)
bpy.ops.mesh.primitive_plane_add(size=6)
bpy.data.objects['Plane'].color = (0.2, 0.3, 0.35, 1.0)
bpy.context.scene.render.engine = 'BLENDER_WORKBENCH'
bpy.context.scene.display.shading.light = 'FLAT'
bpy.context.scene.display.shading.color_type = 'OBJECT'
bpy.context.scene.render.resolution_x = 640
bpy.context.scene.render.resolution_y = 360
bpy.context.scene.render.resolution_percentage = 50
bpy.context.scene.render.image_settings.color_mode = 'RGB'
bpy.context.scene.render.filepath = 'out/{scene_name}_{resolution_x}x{resolution_y}'
#@render bpy.ops.render.render(write_still=True)
