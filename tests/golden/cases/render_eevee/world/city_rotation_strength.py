# golden: startup=factory compare=objects tier=T0
# render-golden: image=out/city_rotation_strength.png min_ssim=0.90
# WP 5.2 HDRI world behind a Mapping node: Blender's bundled `city.exr` (CC0, Poly Haven) turned
# by Texture Coordinate (Generated) -> Mapping (Z rotation 2.0) -> Environment Texture, at
# Background strength 0.7. Its sun (35456 at strength 1) becomes the world sun light and casts
# the spheres' shadows on the ground plane; rough metal, chrome and white diffuse spheres.
import bpy
bpy.ops.object.delete()
bpy.data.lights['Light'].energy = 0.0
bpy.data.images.load('city.exr')
bpy.data.worlds['World'].node_tree.nodes.new('ShaderNodeTexEnvironment')
bpy.data.worlds['World'].node_tree.nodes.new('ShaderNodeTexCoord')
bpy.data.worlds['World'].node_tree.nodes.new('ShaderNodeMapping')
bpy.data.worlds['World'].node_tree.nodes['Environment Texture'].image = bpy.data.images['city.exr']
bpy.data.worlds['World'].node_tree.links.new(bpy.data.worlds['World'].node_tree.nodes['Texture Coordinate'].outputs['Generated'], bpy.data.worlds['World'].node_tree.nodes['Mapping'].inputs['Vector'])
bpy.data.worlds['World'].node_tree.links.new(bpy.data.worlds['World'].node_tree.nodes['Mapping'].outputs['Vector'], bpy.data.worlds['World'].node_tree.nodes['Environment Texture'].inputs['Vector'])
bpy.data.worlds['World'].node_tree.links.new(bpy.data.worlds['World'].node_tree.nodes['Environment Texture'].outputs['Color'], bpy.data.worlds['World'].node_tree.nodes['Background'].inputs['Color'])
bpy.data.worlds['World'].node_tree.nodes['Mapping'].inputs['Rotation'].default_value = (0.0, 0.0, 2.0)
bpy.data.worlds['World'].node_tree.nodes['Background'].inputs['Strength'].default_value = 0.7
bpy.ops.mesh.primitive_plane_add(size=12.0, location=(0.0, 0.0, 0.0))
bpy.ops.mesh.primitive_uv_sphere_add(segments=64, ring_count=32, radius=0.8, location=(-2.0, 0.0, 0.8))
bpy.ops.object.shade_smooth()
bpy.ops.mesh.primitive_uv_sphere_add(segments=64, ring_count=32, radius=0.8, location=(0.0, 0.0, 0.8))
bpy.ops.object.shade_smooth()
bpy.ops.mesh.primitive_uv_sphere_add(segments=64, ring_count=32, radius=0.8, location=(2.0, 0.0, 0.8))
bpy.ops.object.shade_smooth()
bpy.data.materials.new('Ground')
bpy.data.materials['Ground'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.35, 0.33, 0.3, 1.0)
bpy.data.materials['Ground'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.8
bpy.data.objects['Plane'].data.materials.append(bpy.data.materials['Ground'])
bpy.data.materials.new('RoughMetal')
bpy.data.materials['RoughMetal'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.9, 0.5, 0.35, 1.0)
bpy.data.materials['RoughMetal'].node_tree.nodes['Principled BSDF'].inputs['Metallic'].default_value = 1.0
bpy.data.materials['RoughMetal'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.6
bpy.data.objects['Sphere'].data.materials.append(bpy.data.materials['RoughMetal'])
bpy.data.materials.new('Chrome')
bpy.data.materials['Chrome'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.9, 0.9, 0.9, 1.0)
bpy.data.materials['Chrome'].node_tree.nodes['Principled BSDF'].inputs['Metallic'].default_value = 1.0
bpy.data.materials['Chrome'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.05
bpy.data.objects['Sphere.001'].data.materials.append(bpy.data.materials['Chrome'])
bpy.data.materials.new('White')
bpy.data.materials['White'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.9, 0.9, 0.9, 1.0)
bpy.data.materials['White'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.7
bpy.data.objects['Sphere.002'].data.materials.append(bpy.data.materials['White'])
bpy.data.objects['Camera'].location = (0.0, -10.0, 3.5)
bpy.data.objects['Camera'].rotation_euler = (1.2566, 0.0, 0.0)
bpy.context.scene.render.engine = 'BLENDER_EEVEE'
bpy.context.scene.render.resolution_x = 320
bpy.context.scene.render.resolution_y = 180
bpy.context.scene.view_settings.view_transform = 'Standard'
bpy.context.scene.eevee.taa_render_samples = 16
bpy.context.scene.render.filepath = 'out/city_rotation_strength'
#@render bpy.ops.render.render(write_still=True)
