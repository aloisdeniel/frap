# golden: startup=factory compare=objects tier=T0
# render-golden: image=out/clearcoat.png min_ssim=0.985 max_delta_e=0.5
# EEVEE shading feature golden (WP 5.5): clearcoat. Three rough red spheres (Roughness 0.6)
# with Coat Weight 0, 0.5 (yellow Coat Tint) and 1 (Coat IOR 2), Coat Roughness 0.05: the coat
# adds a sharp highlight of the point light and of the world, and darkens (and tints) the
# layer below by its Fresnel share.
import bpy
bpy.data.objects.remove(bpy.data.objects['Cube'])
bpy.ops.mesh.primitive_uv_sphere_add(segments=48, ring_count=24, radius=0.8, location=(-1.9, 0.0, 0.0))
bpy.ops.object.shade_smooth()
bpy.data.materials.new('Coat0')
bpy.data.materials['Coat0'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.7, 0.05, 0.05, 1.0)
bpy.data.materials['Coat0'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.6
bpy.context.object.data.materials.append(bpy.data.materials['Coat0'])
bpy.ops.mesh.primitive_uv_sphere_add(segments=48, ring_count=24, radius=0.8, location=(0.0, 0.0, 0.0))
bpy.ops.object.shade_smooth()
bpy.data.materials.new('Coat5')
bpy.data.materials['Coat5'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.7, 0.05, 0.05, 1.0)
bpy.data.materials['Coat5'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.6
bpy.data.materials['Coat5'].node_tree.nodes['Principled BSDF'].inputs['Coat Weight'].default_value = 0.5
bpy.data.materials['Coat5'].node_tree.nodes['Principled BSDF'].inputs['Coat Roughness'].default_value = 0.05
bpy.data.materials['Coat5'].node_tree.nodes['Principled BSDF'].inputs['Coat Tint'].default_value = (0.9, 0.9, 0.4, 1.0)
bpy.context.object.data.materials.append(bpy.data.materials['Coat5'])
bpy.ops.mesh.primitive_uv_sphere_add(segments=48, ring_count=24, radius=0.8, location=(1.9, 0.0, 0.0))
bpy.ops.object.shade_smooth()
bpy.data.materials.new('Coat1')
bpy.data.materials['Coat1'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.7, 0.05, 0.05, 1.0)
bpy.data.materials['Coat1'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.6
bpy.data.materials['Coat1'].node_tree.nodes['Principled BSDF'].inputs['Coat Weight'].default_value = 1.0
bpy.data.materials['Coat1'].node_tree.nodes['Principled BSDF'].inputs['Coat Roughness'].default_value = 0.05
bpy.data.materials['Coat1'].node_tree.nodes['Principled BSDF'].inputs['Coat IOR'].default_value = 2.0
bpy.context.object.data.materials.append(bpy.data.materials['Coat1'])
bpy.data.worlds['World'].node_tree.nodes['Background'].inputs['Color'].default_value = (0.3, 0.3, 0.35, 1.0)
bpy.data.objects['Camera'].location = (0.0, -6.5, 1.5)
bpy.data.objects['Camera'].rotation_euler = (1.35, 0.0, 0.0)
bpy.data.objects['Light'].location = (1.0, -4.0, 3.0)
bpy.data.lights['Light'].shadow_soft_size = 0.1
bpy.context.scene.render.engine = 'BLENDER_EEVEE'
bpy.context.scene.render.resolution_x = 200
bpy.context.scene.render.resolution_y = 100
bpy.context.scene.view_settings.view_transform = 'Standard'
bpy.context.scene.eevee.taa_render_samples = 16
bpy.context.scene.render.filepath = 'out/clearcoat'
#@render bpy.ops.render.render(write_still=True)
