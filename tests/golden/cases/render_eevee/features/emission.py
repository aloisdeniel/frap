# golden: startup=factory compare=objects tier=T0
# render-golden: image=out/emission.png min_ssim=0.88
# EEVEE shading feature golden (WP 5.5): emission. An orange sphere emitting at strength 2, a
# cyan one at strength 6 (above 1: glTF's emissive strength split), and a plane whose
# Emission Color is a generated UV_GRID image at strength 1, in a dark world.
import bpy
bpy.data.objects.remove(bpy.data.objects['Cube'])
bpy.data.lights['Light'].energy = 100.0
bpy.ops.mesh.primitive_uv_sphere_add(segments=32, ring_count=16, radius=0.7, location=(-1.6, 0.0, 0.8))
bpy.ops.object.shade_smooth()
bpy.data.materials.new('Orange')
bpy.data.materials['Orange'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.1, 0.1, 0.1, 1.0)
bpy.data.materials['Orange'].node_tree.nodes['Principled BSDF'].inputs['Emission Color'].default_value = (1.0, 0.4, 0.05, 1.0)
bpy.data.materials['Orange'].node_tree.nodes['Principled BSDF'].inputs['Emission Strength'].default_value = 2.0
bpy.context.object.data.materials.append(bpy.data.materials['Orange'])
bpy.ops.mesh.primitive_uv_sphere_add(segments=32, ring_count=16, radius=0.7, location=(1.6, 0.0, 0.8))
bpy.ops.object.shade_smooth()
bpy.data.materials.new('Cyan')
bpy.data.materials['Cyan'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.1, 0.1, 0.1, 1.0)
bpy.data.materials['Cyan'].node_tree.nodes['Principled BSDF'].inputs['Emission Color'].default_value = (0.1, 0.6, 0.8, 1.0)
bpy.data.materials['Cyan'].node_tree.nodes['Principled BSDF'].inputs['Emission Strength'].default_value = 6.0
bpy.context.object.data.materials.append(bpy.data.materials['Cyan'])
bpy.ops.image.new(name='Grid', width=64, height=64, generated_type='UV_GRID')
bpy.ops.mesh.primitive_plane_add(size=2.0, location=(0.0, 0.5, -0.2))
bpy.data.materials.new('Screen')
bpy.data.materials['Screen'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.0, 0.0, 0.0, 1.0)
bpy.data.materials['Screen'].node_tree.nodes['Principled BSDF'].inputs['Emission Strength'].default_value = 1.0
bpy.data.materials['Screen'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Screen'].node_tree.nodes['Image Texture'].image = bpy.data.images['Grid']
bpy.data.materials['Screen'].node_tree.links.new(bpy.data.materials['Screen'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Screen'].node_tree.nodes['Principled BSDF'].inputs['Emission Color'])
bpy.context.object.data.materials.append(bpy.data.materials['Screen'])
bpy.data.worlds['World'].node_tree.nodes['Background'].inputs['Color'].default_value = (0.01, 0.01, 0.01, 1.0)
bpy.data.objects['Camera'].location = (0.0, -6.0, 4.0)
bpy.data.objects['Camera'].rotation_euler = (0.98, 0.0, 0.0)
bpy.context.scene.render.engine = 'BLENDER_EEVEE'
bpy.context.scene.render.resolution_x = 200
bpy.context.scene.render.resolution_y = 120
bpy.context.scene.view_settings.view_transform = 'Standard'
bpy.context.scene.eevee.taa_render_samples = 16
bpy.context.scene.render.filepath = 'out/emission'
#@render bpy.ops.render.render(write_still=True)
