# golden: startup=factory compare=objects,context tier=T0
# render-golden: image=out/cube_factory.png
# Render golden (WP 1.24): the factory cube through the factory camera, Workbench SOLID +
# STUDIO + MATERIAL, AgX. `#@render` lines run only in crates/frap-ops-render-tests (they need
# a GPU); the generic golden suite compares the scene state with Blender's.
import bpy
bpy.context.scene.render.engine = 'BLENDER_WORKBENCH'
bpy.context.scene.render.resolution_x = 320
bpy.context.scene.render.resolution_y = 180
bpy.context.scene.render.filepath = 'out/cube_factory'
#@render bpy.ops.render.render(write_still=True)
