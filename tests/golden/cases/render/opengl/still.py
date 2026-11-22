# golden: startup=factory compare=objects,context tier=T0
# render-golden: image=out/opengl_still.png
# WP 8.3.6: render.opengl (Viewport Render, headless-extended, D-160) renders the scene camera with
# Workbench and the scene display settings, whatever scene.render.engine says (EEVEE here), and
# write_still writes render.filepath like render.render. Blender cannot run it in `-b` ('Cannot
# use OpenGL render in background mode'), so its reference is the Workbench render of the same
# scene. `#@opengl` lines run in frap and `#@opengl-blender` lines in Blender, both only in
# crates/frap-ops-render-tests/tests/render_opengl.rs (they need a GPU); the generic golden
# suite compares the scene state.
import bpy
bpy.ops.mesh.primitive_monkey_add(location=(-1.6, 0.4, 0.6), size=1.2)
bpy.ops.object.shade_smooth()
bpy.data.objects['Cube'].scale = (0.8, 0.8, 0.8)
bpy.context.scene.render.resolution_x = 320
bpy.context.scene.render.resolution_y = 180
bpy.context.scene.display.shading.color_type = 'OBJECT'
bpy.data.objects['Suzanne'].color = (0.8, 0.3, 0.1, 1.0)
bpy.context.scene.render.filepath = 'out/opengl_still'
#@opengl bpy.ops.render.opengl(write_still=True)
#@opengl-blender bpy.context.scene.render.engine = 'BLENDER_WORKBENCH'
#@opengl-blender bpy.ops.render.render(write_still=True)
