# golden: startup=factory compare=objects,materials,context tier=T0
# render-golden: image=out/film_transparent_rgba.png
# M5 exit scene (WP 5.6): `render.film_transparent` with RGBA PNG output: Suzanne (flat shaded)
# and a sphere with no floor over a transparent background (alpha compared as its own channel),
# the world still lighting them, Standard view transform.
# Dither off (`render.dither_intensity = 0`); `night_low_contrast_dither.py` says why.
import bpy
bpy.ops.object.delete()
bpy.ops.mesh.primitive_monkey_add(size=1.8, location=(-0.9, 0.0, 0.0))
bpy.data.materials.new('Green')
bpy.data.materials['Green'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.1, 0.5, 0.1, 1.0)
bpy.data.materials['Green'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.4
bpy.context.object.data.materials.append(bpy.data.materials['Green'])
bpy.ops.mesh.primitive_uv_sphere_add(radius=0.8, location=(1.2, 0.5, 0.0))
bpy.ops.object.shade_smooth()
bpy.data.materials.new('Plum')
bpy.data.materials['Plum'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.4, 0.1, 0.4, 1.0)
bpy.data.materials['Plum'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.3
bpy.context.object.data.materials.append(bpy.data.materials['Plum'])
bpy.data.worlds['World'].node_tree.nodes['Background'].inputs['Color'].default_value = (0.4, 0.4, 0.45, 1.0)
bpy.data.worlds['World'].node_tree.nodes['Background'].inputs['Strength'].default_value = 1.0
bpy.context.scene.render.engine = 'BLENDER_EEVEE'
bpy.context.scene.render.resolution_x = 320
bpy.context.scene.render.resolution_y = 180
bpy.context.scene.eevee.taa_render_samples = 16
bpy.context.scene.view_settings.view_transform = 'Standard'
bpy.context.scene.render.film_transparent = True
bpy.context.scene.render.image_settings.color_mode = 'RGBA'
bpy.context.scene.render.dither_intensity = 0.0
bpy.context.scene.render.filepath = 'out/film_transparent_rgba'
#@render bpy.ops.render.render(write_still=True)
