# golden: startup=factory compare=objects tier=T0
# render-golden: image=out/courtyard_roughness_row.png min_ssim=0.90
# WP 5.2 HDRI reflections per roughness: Blender's bundled `courtyard.exr` (CC0, Poly Haven)
# reflected by five metallic spheres of roughness 0, 0.2, 0.4, 0.6 and 0.9 (the probe mips,
# `roughness_to_lod`, and the fade to the L1 world above 0.7). Standard view transform.
import bpy
bpy.ops.object.delete()
bpy.data.lights['Light'].energy = 0.0
bpy.data.images.load('courtyard.exr')
bpy.data.worlds['World'].node_tree.nodes.new('ShaderNodeTexEnvironment')
bpy.data.worlds['World'].node_tree.nodes['Environment Texture'].image = bpy.data.images['courtyard.exr']
bpy.data.worlds['World'].node_tree.links.new(bpy.data.worlds['World'].node_tree.nodes['Environment Texture'].outputs['Color'], bpy.data.worlds['World'].node_tree.nodes['Background'].inputs['Color'])
bpy.ops.mesh.primitive_uv_sphere_add(segments=64, ring_count=32, radius=0.7, location=(-3.2, 0.0, 0.0))
bpy.ops.object.shade_smooth()
bpy.ops.mesh.primitive_uv_sphere_add(segments=64, ring_count=32, radius=0.7, location=(-1.6, 0.0, 0.0))
bpy.ops.object.shade_smooth()
bpy.ops.mesh.primitive_uv_sphere_add(segments=64, ring_count=32, radius=0.7, location=(0.0, 0.0, 0.0))
bpy.ops.object.shade_smooth()
bpy.ops.mesh.primitive_uv_sphere_add(segments=64, ring_count=32, radius=0.7, location=(1.6, 0.0, 0.0))
bpy.ops.object.shade_smooth()
bpy.ops.mesh.primitive_uv_sphere_add(segments=64, ring_count=32, radius=0.7, location=(3.2, 0.0, 0.0))
bpy.ops.object.shade_smooth()
bpy.data.materials.new('R0')
bpy.data.materials['R0'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.95, 0.95, 0.95, 1.0)
bpy.data.materials['R0'].node_tree.nodes['Principled BSDF'].inputs['Metallic'].default_value = 1.0
bpy.data.materials['R0'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.0
bpy.data.objects['Sphere'].data.materials.append(bpy.data.materials['R0'])
bpy.data.materials.new('R2')
bpy.data.materials['R2'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.95, 0.95, 0.95, 1.0)
bpy.data.materials['R2'].node_tree.nodes['Principled BSDF'].inputs['Metallic'].default_value = 1.0
bpy.data.materials['R2'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.2
bpy.data.objects['Sphere.001'].data.materials.append(bpy.data.materials['R2'])
bpy.data.materials.new('R4')
bpy.data.materials['R4'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.95, 0.95, 0.95, 1.0)
bpy.data.materials['R4'].node_tree.nodes['Principled BSDF'].inputs['Metallic'].default_value = 1.0
bpy.data.materials['R4'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.4
bpy.data.objects['Sphere.002'].data.materials.append(bpy.data.materials['R4'])
bpy.data.materials.new('R6')
bpy.data.materials['R6'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.95, 0.95, 0.95, 1.0)
bpy.data.materials['R6'].node_tree.nodes['Principled BSDF'].inputs['Metallic'].default_value = 1.0
bpy.data.materials['R6'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.6
bpy.data.objects['Sphere.003'].data.materials.append(bpy.data.materials['R6'])
bpy.data.materials.new('R9')
bpy.data.materials['R9'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.95, 0.95, 0.95, 1.0)
bpy.data.materials['R9'].node_tree.nodes['Principled BSDF'].inputs['Metallic'].default_value = 1.0
bpy.data.materials['R9'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.9
bpy.data.objects['Sphere.004'].data.materials.append(bpy.data.materials['R9'])
bpy.data.objects['Camera'].location = (0.0, -11.0, 0.8)
bpy.data.objects['Camera'].rotation_euler = (1.4983, 0.0, 0.0)
bpy.context.scene.render.engine = 'BLENDER_EEVEE'
bpy.context.scene.render.resolution_x = 320
bpy.context.scene.render.resolution_y = 180
bpy.context.scene.view_settings.view_transform = 'Standard'
bpy.context.scene.eevee.taa_render_samples = 16
bpy.context.scene.render.filepath = 'out/courtyard_roughness_row'
#@render bpy.ops.render.render(write_still=True)
