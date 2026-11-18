# golden: startup=factory compare=objects,context tier=T0
# render-golden: image=out/shadows_background_table.png min_ssim=0.97 bg_max_delta_e=2
# Render golden (stabilization, M2): Workbench shadows of an edit-mode table (a slab with
# four inset and extruded legs) with no ground plane; the shadow volumes of the legs reach the
# far plane over background pixels, which must match Blender's render pixel for pixel.
import bpy
bpy.data.objects['Cube'].hide_render = True
bpy.ops.mesh.primitive_cube_add(size=2, location=(0, 0, 1), scale=(1.2, 0.8, 0.05))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_by_normal(direction=(0, 0, -1))
bpy.ops.mesh.subdivide(number_cuts=2)
bpy.ops.frap.select_elements(type='FACE', indices=[4, 6, 8, 12])
bpy.ops.mesh.inset(thickness=0.08, use_individual=True)
bpy.ops.mesh.extrude_region_move(TRANSFORM_OT_translate={"value": (0, 0, -0.9)})
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.scene.render.engine = 'BLENDER_WORKBENCH'
bpy.context.scene.display.shading.show_shadows = True
bpy.context.scene.render.resolution_x = 480
bpy.context.scene.render.resolution_y = 320
bpy.context.scene.render.filepath = 'out/shadows_background_table'
#@render bpy.ops.render.render(write_still=True)
