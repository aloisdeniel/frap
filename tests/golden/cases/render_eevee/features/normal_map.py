# golden: startup=factory compare=objects tier=T0
# render-golden: image=out/normal_map.png min_ssim=0.85
# EEVEE shading feature golden (WP 5.5): a tangent-space normal map (a 4 x 4 grid of bumps,
# `bumps.png`, Non-Color) through a Normal Map node into the Principled BSDF's Normal, on a
# plane and on a smooth UV sphere (MikkTSpace tangents of the UV map), lit by a low sun.
import bpy
bpy.data.objects.remove(bpy.data.objects['Cube'])
bpy.data.lights['Light'].energy = 0.0
bpy.data.images.load('bumps.png')
bpy.data.images['bumps.png'].colorspace_settings.name = 'Non-Color'
bpy.data.materials.new('Bumpy')
bpy.data.materials['Bumpy'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.8, 0.8, 0.8, 1.0)
bpy.data.materials['Bumpy'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.6
bpy.data.materials['Bumpy'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Bumpy'].node_tree.nodes['Image Texture'].image = bpy.data.images['bumps.png']
bpy.data.materials['Bumpy'].node_tree.nodes.new('ShaderNodeNormalMap')
bpy.data.materials['Bumpy'].node_tree.nodes['Normal Map'].inputs['Strength'].default_value = 1.0
bpy.data.materials['Bumpy'].node_tree.links.new(bpy.data.materials['Bumpy'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Bumpy'].node_tree.nodes['Normal Map'].inputs['Color'])
bpy.data.materials['Bumpy'].node_tree.links.new(bpy.data.materials['Bumpy'].node_tree.nodes['Normal Map'].outputs['Normal'], bpy.data.materials['Bumpy'].node_tree.nodes['Principled BSDF'].inputs['Normal'])
bpy.ops.mesh.primitive_plane_add(size=4.0, location=(-1.6, 0.0, 0.0))
bpy.context.object.data.materials.append(bpy.data.materials['Bumpy'])
bpy.ops.mesh.primitive_uv_sphere_add(segments=48, ring_count=24, radius=1.2, location=(1.8, 0.0, 1.0))
bpy.ops.object.shade_smooth()
bpy.context.object.data.materials.append(bpy.data.materials['Bumpy'])
bpy.ops.object.light_add(type='SUN', rotation=(0.0, 1.1, 0.6))
bpy.context.object.data.energy = 4.0
bpy.data.objects['Camera'].location = (0.0, -7.5, 6.0)
bpy.data.objects['Camera'].rotation_euler = (0.9, 0.0, 0.0)
bpy.context.scene.render.engine = 'BLENDER_EEVEE'
bpy.context.scene.render.resolution_x = 200
bpy.context.scene.render.resolution_y = 120
bpy.context.scene.view_settings.view_transform = 'Standard'
bpy.context.scene.eevee.taa_render_samples = 16
bpy.context.scene.render.filepath = 'out/normal_map'
#@render bpy.ops.render.render(write_still=True)
