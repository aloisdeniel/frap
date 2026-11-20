# golden: startup=factory compare=objects,materials,context tier=T0
# render-golden: image=out/hdri_sunset_metals.png
# M5 exit scene (WP 5.6): Blender's bundled `sunset.exr` (CC0, Poly Haven) as the world
# (Environment Texture, strength 1.2, Mapping-free) lighting a metallic Suzanne (roughness 0.2, subdivided, smooth) and
# a dielectric sphere (roughness 0.1), AgX view transform.
# Dither off (`render.dither_intensity = 0`): two renders' 8-bit dither noise is uncorrelated.
import bpy
bpy.ops.object.delete()
bpy.data.lights['Light'].energy = 0.0
bpy.data.images.load('sunset.exr')
bpy.data.worlds['World'].node_tree.nodes.new('ShaderNodeTexEnvironment')
bpy.data.worlds['World'].node_tree.nodes['Environment Texture'].image = bpy.data.images['sunset.exr']
bpy.data.worlds['World'].node_tree.links.new(bpy.data.worlds['World'].node_tree.nodes['Environment Texture'].outputs['Color'], bpy.data.worlds['World'].node_tree.nodes['Background'].inputs['Color'])
bpy.data.worlds['World'].node_tree.nodes['Background'].inputs['Strength'].default_value = 1.2
bpy.ops.mesh.primitive_monkey_add(size=1.6, location=(-1.0, 0.0, 0.0), rotation=(0.0, 0.0, -0.4))
bpy.ops.object.shade_smooth()
bpy.ops.object.modifier_add(type='SUBSURF')
bpy.data.materials.new('Chrome')
bpy.data.materials['Chrome'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.9, 0.9, 0.9, 1.0)
bpy.data.materials['Chrome'].node_tree.nodes['Principled BSDF'].inputs['Metallic'].default_value = 1.0
bpy.data.materials['Chrome'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.2
bpy.context.object.data.materials.append(bpy.data.materials['Chrome'])
bpy.ops.mesh.primitive_uv_sphere_add(segments=64, ring_count=32, radius=0.7, location=(1.1, 0.0, 0.0))
bpy.ops.object.shade_smooth()
bpy.data.materials.new('Glaze')
bpy.data.materials['Glaze'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.1, 0.35, 0.15, 1.0)
bpy.data.materials['Glaze'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.1
bpy.context.object.data.materials.append(bpy.data.materials['Glaze'])
bpy.data.objects['Camera'].location = (0.0, -6.0, 0.6)
bpy.data.objects['Camera'].rotation_euler = (1.4708, 0.0, 0.0)
bpy.context.scene.render.engine = 'BLENDER_EEVEE'
bpy.context.scene.render.resolution_x = 320
bpy.context.scene.render.resolution_y = 180
bpy.context.scene.eevee.taa_render_samples = 16
bpy.context.scene.view_settings.view_transform = 'AgX'
bpy.context.scene.render.dither_intensity = 0.0
bpy.context.scene.render.filepath = 'out/hdri_sunset_metals'
#@render bpy.ops.render.render(write_still=True)
