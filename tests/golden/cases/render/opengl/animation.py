# golden: startup=factory compare=objects,context tier=T0
# render-golden: image=out/opengl_0003.png
# WP 8.3.6: render.opengl(animation=True) renders frame_start..frame_end by frame_step from the
# scene camera with Workbench (D-160), each frame with its animation, and writes the frame
# paths like render.render(animation=True); frame 3 of the moving cube is compared with
# Blender's Workbench animation render. `#@opengl` / `#@opengl-blender` lines as in still.py.
import bpy
bpy.context.scene.render.resolution_x = 160
bpy.context.scene.render.resolution_y = 90
bpy.context.scene.frame_start = 1
bpy.context.scene.frame_end = 3
bpy.context.scene.frame_step = 2
bpy.data.objects['Cube'].location = (-2.0, 0.0, 0.0)
bpy.data.objects['Cube'].keyframe_insert('location', frame=1)
bpy.data.objects['Cube'].location = (2.0, 0.0, 1.0)
bpy.data.objects['Cube'].keyframe_insert('location', frame=3)
bpy.context.scene.render.filepath = 'out/opengl_'
#@opengl bpy.ops.render.opengl(animation=True)
#@opengl-blender bpy.context.scene.render.engine = 'BLENDER_WORKBENCH'
#@opengl-blender bpy.ops.render.render(animation=True)
