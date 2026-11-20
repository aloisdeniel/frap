# golden: startup=factory compare=objects,materials,context tier=T0
# render-golden: image=out/rgb_point_lights.png
# M5 exit scene (WP 5.6): a white cylinder on a white floor under three coloured point lights
# (red, green, blue; 600 W each, radius 0.1 m), so the shadows are coloured; black world, Khronos
# PBR Neutral view transform.
# Dither off (`render.dither_intensity = 0`); `night_low_contrast_dither.py` says why.
import bpy
bpy.ops.object.delete()
bpy.data.objects.remove(bpy.data.objects['Light'])
bpy.ops.mesh.primitive_cylinder_add(vertices=48, radius=0.6, depth=2.0, location=(0.0, 0.0, 1.0))
bpy.data.materials.new('White')
bpy.data.materials['White'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.8, 0.8, 0.8, 1.0)
bpy.data.materials['White'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.5
bpy.context.object.data.materials.append(bpy.data.materials['White'])
bpy.ops.mesh.primitive_plane_add(size=20.0, location=(0.0, 0.0, 0.0))
bpy.context.object.data.materials.append(bpy.data.materials['White'])
bpy.ops.object.light_add(type='POINT', location=(2.5, 0.0, 3.0))
bpy.context.object.data.energy = 600.0
bpy.context.object.data.color = (1.0, 0.1, 0.1)
bpy.context.object.data.shadow_soft_size = 0.1
bpy.ops.object.light_add(type='POINT', location=(-1.25, 2.17, 3.0))
bpy.context.object.data.energy = 600.0
bpy.context.object.data.color = (0.1, 1.0, 0.1)
bpy.context.object.data.shadow_soft_size = 0.1
bpy.ops.object.light_add(type='POINT', location=(-1.25, -2.17, 3.0))
bpy.context.object.data.energy = 600.0
bpy.context.object.data.color = (0.1, 0.1, 1.0)
bpy.context.object.data.shadow_soft_size = 0.1
bpy.data.worlds['World'].node_tree.nodes['Background'].inputs['Strength'].default_value = 0.0
bpy.context.scene.render.engine = 'BLENDER_EEVEE'
bpy.context.scene.render.resolution_x = 320
bpy.context.scene.render.resolution_y = 180
bpy.context.scene.eevee.taa_render_samples = 16
bpy.context.scene.view_settings.view_transform = 'Khronos PBR Neutral'
bpy.context.scene.render.dither_intensity = 0.0
bpy.context.scene.render.filepath = 'out/rgb_point_lights'
#@render bpy.ops.render.render(write_still=True)
