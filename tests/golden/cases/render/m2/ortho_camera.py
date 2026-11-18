# golden: startup=factory compare=objects,context tier=T0
# render-golden: image=out/ortho_camera.png min_ssim=0.95
# Render golden (WP 2.3): the kind of orthographic camera `frap.render_contact_sheet` reports
# for its tiles (here the TOP-ish view of a small scene, square output), rendered by a real
# camera object with shadows and an outline.
import bpy
bpy.ops.mesh.primitive_plane_add(size=6)
bpy.ops.mesh.primitive_uv_sphere_add(radius=0.6, location=(1.5, 0.5, 0.8))
bpy.ops.object.shade_smooth()
bpy.data.cameras['Camera'].type = 'ORTHO'
bpy.data.cameras['Camera'].ortho_scale = 6.6
bpy.data.cameras['Camera'].clip_start = 0.8
bpy.data.cameras['Camera'].clip_end = 9.2
bpy.data.objects['Camera'].location = (0.0, -4.6, 0.5)
bpy.data.objects['Camera'].rotation_euler = (1.5707963, 0.0, 0.0)
bpy.context.scene.render.engine = 'BLENDER_WORKBENCH'
bpy.context.scene.render.resolution_x = 192
bpy.context.scene.render.resolution_y = 192
bpy.context.scene.display.shading.show_shadows = True
bpy.context.scene.display.shading.show_object_outline = True
bpy.context.scene.render.filepath = 'out/ortho_camera'
#@render bpy.ops.render.render(write_still=True)
