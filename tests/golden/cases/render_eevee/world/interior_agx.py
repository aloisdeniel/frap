# golden: startup=factory compare=objects tier=T0
# render-golden: image=out/interior_agx.png min_ssim=0.90
# WP 5.2 HDRI world with the factory view transform (AgX): Blender's bundled `interior.exr`
# (CC0, Poly Haven; bright window values up to 35744 pass the sun threshold) at Background
# strength 1.5 lighting dielectric spheres of roughness 0.05, 0.35 and 1.0 (Base Color blue,
# yellow, white) and one with the default material.
import bpy
bpy.ops.object.delete()
bpy.data.lights['Light'].energy = 0.0
bpy.data.images.load('interior.exr')
bpy.data.worlds['World'].node_tree.nodes.new('ShaderNodeTexEnvironment')
bpy.data.worlds['World'].node_tree.nodes['Environment Texture'].image = bpy.data.images['interior.exr']
bpy.data.worlds['World'].node_tree.links.new(bpy.data.worlds['World'].node_tree.nodes['Environment Texture'].outputs['Color'], bpy.data.worlds['World'].node_tree.nodes['Background'].inputs['Color'])
bpy.data.worlds['World'].node_tree.nodes['Background'].inputs['Strength'].default_value = 1.5
bpy.ops.mesh.primitive_uv_sphere_add(segments=64, ring_count=32, radius=0.75, location=(-2.4, 0.0, 0.0))
bpy.ops.object.shade_smooth()
bpy.ops.mesh.primitive_uv_sphere_add(segments=64, ring_count=32, radius=0.75, location=(-0.8, 0.0, 0.0))
bpy.ops.object.shade_smooth()
bpy.ops.mesh.primitive_uv_sphere_add(segments=64, ring_count=32, radius=0.75, location=(0.8, 0.0, 0.0))
bpy.ops.object.shade_smooth()
bpy.ops.mesh.primitive_uv_sphere_add(segments=64, ring_count=32, radius=0.75, location=(2.4, 0.0, 0.0))
bpy.ops.object.shade_smooth()
bpy.data.materials.new('Blue')
bpy.data.materials['Blue'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.05, 0.15, 0.6, 1.0)
bpy.data.materials['Blue'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.05
bpy.data.objects['Sphere'].data.materials.append(bpy.data.materials['Blue'])
bpy.data.materials.new('Yellow')
bpy.data.materials['Yellow'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.8, 0.6, 0.1, 1.0)
bpy.data.materials['Yellow'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.35
bpy.data.objects['Sphere.001'].data.materials.append(bpy.data.materials['Yellow'])
bpy.data.materials.new('Chalk')
bpy.data.materials['Chalk'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.9, 0.9, 0.9, 1.0)
bpy.data.materials['Chalk'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 1.0
bpy.data.objects['Sphere.002'].data.materials.append(bpy.data.materials['Chalk'])
bpy.data.objects['Camera'].location = (0.0, -9.5, 0.5)
bpy.data.objects['Camera'].rotation_euler = (1.5184, 0.0, 0.0)
bpy.context.scene.render.engine = 'BLENDER_EEVEE'
bpy.context.scene.render.resolution_x = 320
bpy.context.scene.render.resolution_y = 180
bpy.context.scene.eevee.taa_render_samples = 16
bpy.context.scene.render.filepath = 'out/interior_agx'
#@render bpy.ops.render.render(write_still=True)
