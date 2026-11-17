# golden: startup=factory compare=objects,context tier=T0
# render.render errors (WP 1.24), in Blender's check order; none of them renders, so the case
# needs no GPU: frame range before size before camera before output-path templates.
import bpy
bpy.context.scene.render.engine = 'BLENDER_WORKBENCH'
bpy.context.scene.render.resolution_x = 16
bpy.context.scene.render.resolution_y = 8
bpy.context.scene.camera = None
bpy.ops.render.render()
bpy.ops.render.render(write_still=True)
bpy.ops.render.render(animation=True)
bpy.ops.render.render(animation=True, frame_start=300)
bpy.ops.render.render(animation=True, frame_start=5, frame_end=4)
bpy.context.scene.render.resolution_percentage = 1
bpy.ops.render.render()
bpy.context.scene.render.resolution_percentage = 100
bpy.context.scene.camera = bpy.data.objects['Camera']
bpy.context.scene.render.filepath = 'out/{nope}'
bpy.ops.render.render(write_still=True)
bpy.context.scene.render.filepath = 'out/{scene_name:##}{resolution_x'
bpy.ops.render.render(write_still=True)
bpy.context.scene.render.filepath = 'out/x}'
bpy.ops.render.render(animation=True, frame_start=1, frame_end=1)
bpy.context.scene.render.resolution_percentage = 1
bpy.ops.render.render(write_still=True)
