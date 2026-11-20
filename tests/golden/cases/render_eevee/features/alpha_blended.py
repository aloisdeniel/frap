# golden: startup=factory compare=objects tier=T0
# render-golden: image=out/alpha_blended.png min_ssim=0.88
# EEVEE shading feature golden (WP 5.5): alpha blending. A red opaque plane behind a blue
# plane (Alpha 0.4) and a yellow sphere (Alpha 0.6), both `surface_render_method = 'BLENDED'`:
# drawn after the opaque surfaces, back to front, blended over what is behind.
import bpy
bpy.data.objects.remove(bpy.data.objects['Cube'])
bpy.ops.mesh.primitive_plane_add(size=4.0, location=(0.0, 2.0, 0.0), rotation=(1.5708, 0.0, 0.0))
bpy.data.materials.new('Back')
bpy.data.materials['Back'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.8, 0.05, 0.05, 1.0)
bpy.context.object.data.materials.append(bpy.data.materials['Back'])
bpy.ops.mesh.primitive_plane_add(size=2.0, location=(-0.8, 0.0, 0.0), rotation=(1.5708, 0.0, 0.0))
bpy.data.materials.new('Glass')
bpy.data.materials['Glass'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.05, 0.2, 0.9, 1.0)
bpy.data.materials['Glass'].node_tree.nodes['Principled BSDF'].inputs['Alpha'].default_value = 0.4
bpy.data.materials['Glass'].surface_render_method = 'BLENDED'
bpy.context.object.data.materials.append(bpy.data.materials['Glass'])
bpy.ops.mesh.primitive_uv_sphere_add(segments=32, ring_count=16, radius=0.8, location=(0.9, -0.5, 0.0))
bpy.ops.object.shade_smooth()
bpy.data.materials.new('Ball')
bpy.data.materials['Ball'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.9, 0.8, 0.1, 1.0)
bpy.data.materials['Ball'].node_tree.nodes['Principled BSDF'].inputs['Alpha'].default_value = 0.6
bpy.data.materials['Ball'].surface_render_method = 'BLENDED'
bpy.context.object.data.materials.append(bpy.data.materials['Ball'])
bpy.data.objects['Camera'].location = (0.0, -7.0, 0.5)
bpy.data.objects['Camera'].rotation_euler = (1.5, 0.0, 0.0)
bpy.data.objects['Light'].location = (2.0, -4.0, 4.0)
bpy.context.scene.render.engine = 'BLENDER_EEVEE'
bpy.context.scene.render.resolution_x = 160
bpy.context.scene.render.resolution_y = 120
bpy.context.scene.view_settings.view_transform = 'Standard'
bpy.context.scene.eevee.taa_render_samples = 16
bpy.context.scene.render.filepath = 'out/alpha_blended'
#@render bpy.ops.render.render(write_still=True)
