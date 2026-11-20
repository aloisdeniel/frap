# golden: startup=factory compare=objects tier=T0
# render-golden: image=out/forest_spheres.png min_ssim=0.90
# WP 5.2 HDRI world: Blender's bundled `forest.exr` (CC0, Poly Haven, from
# release/datafiles/studiolights/world) as the only light, seen behind a diffuse, a glossy
# dielectric and a metallic sphere. The HDRI's sun (up to 1437) passes `World.sun_threshold`
# (10) and becomes the world sun light; the rest is the L1 diffuse world and the prefiltered
# reflection probe. EEVEE render, Standard view transform.
import bpy
bpy.ops.object.delete()
bpy.data.lights['Light'].energy = 0.0
bpy.data.images.load('forest.exr')
bpy.data.worlds['World'].node_tree.nodes.new('ShaderNodeTexEnvironment')
bpy.data.worlds['World'].node_tree.nodes['Environment Texture'].image = bpy.data.images['forest.exr']
bpy.data.worlds['World'].node_tree.links.new(bpy.data.worlds['World'].node_tree.nodes['Environment Texture'].outputs['Color'], bpy.data.worlds['World'].node_tree.nodes['Background'].inputs['Color'])
bpy.ops.mesh.primitive_uv_sphere_add(segments=64, ring_count=32, radius=0.8, location=(-2.0, 0.0, 0.0))
bpy.ops.object.shade_smooth()
bpy.ops.mesh.primitive_uv_sphere_add(segments=64, ring_count=32, radius=0.8, location=(0.0, 0.0, 0.0))
bpy.ops.object.shade_smooth()
bpy.ops.mesh.primitive_uv_sphere_add(segments=64, ring_count=32, radius=0.8, location=(2.0, 0.0, 0.0))
bpy.ops.object.shade_smooth()
bpy.data.materials.new('Diffuse')
bpy.data.materials['Diffuse'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.8, 0.8, 0.8, 1.0)
bpy.data.materials['Diffuse'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.9
bpy.data.objects['Sphere'].data.materials.append(bpy.data.materials['Diffuse'])
bpy.data.materials.new('Glossy')
bpy.data.materials['Glossy'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.6, 0.05, 0.03, 1.0)
bpy.data.materials['Glossy'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.15
bpy.data.objects['Sphere.001'].data.materials.append(bpy.data.materials['Glossy'])
bpy.data.materials.new('Metal')
bpy.data.materials['Metal'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.95, 0.75, 0.4, 1.0)
bpy.data.materials['Metal'].node_tree.nodes['Principled BSDF'].inputs['Metallic'].default_value = 1.0
bpy.data.materials['Metal'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.3
bpy.data.objects['Sphere.002'].data.materials.append(bpy.data.materials['Metal'])
bpy.data.objects['Camera'].location = (0.0, -9.0, 1.2)
bpy.data.objects['Camera'].rotation_euler = (1.4382, 0.0, 0.0)
bpy.context.scene.render.engine = 'BLENDER_EEVEE'
bpy.context.scene.render.resolution_x = 320
bpy.context.scene.render.resolution_y = 180
bpy.context.scene.view_settings.view_transform = 'Standard'
bpy.context.scene.eevee.taa_render_samples = 16
bpy.context.scene.render.filepath = 'out/forest_spheres'
#@render bpy.ops.render.render(write_still=True)
