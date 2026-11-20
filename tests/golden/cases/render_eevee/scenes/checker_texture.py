# golden: startup=factory compare=objects,materials,context tier=T0
# render-golden: image=out/checker_texture.png
# M5 exit scene (WP 5.6): an image texture (`checker.png`, an 8 x 8 sRGB checker with coloured
# squares, Linear interpolation) on the Base Color of a cube and of the floor, lit by the factory
# point light and a fill sun, Standard view transform.
# Dither off (`render.dither_intensity = 0`): two renders' 8-bit dither noise is uncorrelated.
import bpy
bpy.ops.object.delete()
bpy.data.images.load('checker.png')
bpy.data.materials.new('Checker')
bpy.data.materials['Checker'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.5
bpy.data.materials['Checker'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Checker'].node_tree.nodes['Image Texture'].image = bpy.data.images['checker.png']
bpy.data.materials['Checker'].node_tree.links.new(bpy.data.materials['Checker'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Checker'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.ops.mesh.primitive_cube_add(size=2.0, location=(0.0, 0.0, 1.0), rotation=(0.0, 0.0, 0.3))
bpy.context.object.data.materials.append(bpy.data.materials['Checker'])
bpy.ops.mesh.primitive_plane_add(size=8.0, location=(0.0, 0.0, 0.0))
bpy.context.object.data.materials.append(bpy.data.materials['Checker'])
bpy.ops.object.light_add(type='SUN', location=(0.0, 0.0, 5.0), rotation=(0.5, -0.4, 0.0))
bpy.context.object.data.energy = 1.5
bpy.context.scene.render.engine = 'BLENDER_EEVEE'
bpy.context.scene.render.resolution_x = 320
bpy.context.scene.render.resolution_y = 180
bpy.context.scene.eevee.taa_render_samples = 16
bpy.context.scene.view_settings.view_transform = 'Standard'
bpy.context.scene.render.dither_intensity = 0.0
bpy.context.scene.render.filepath = 'out/checker_texture'
#@render bpy.ops.render.render(write_still=True)
