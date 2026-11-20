# golden: startup=factory compare=objects,materials,context tier=T0
# render-golden: image=out/night_low_contrast_dither.png
# M5 exit scene (WP 5.6): the default dither on dark, low-contrast content. Grey shapes on a grey
# floor at night: a dim blue world (strength 0.15), a weak warm AREA light (DISK, 2 m), the
# Standard view transform (no look) with exposure -1. Smooth dark gradients are where the 8-bit
# dither matters most; this case and `factory_cube.py` keep Blender's default
# `render.dither_intensity` (1.0) on both sides. frap ports `dither_random_value`, so the noise is
# keyed by pixel exactly as in Blender (measured: the ±1 patterns correlate at 0.67, 0.008 when
# misaligned); only pixels whose float values differ by a fraction of a level flip by one level.
# The other scenes turn dither off because the comparator's SSIM (image-compare 0.5.0 sums the
# 8 x 8 window covariance instead of averaging it, so its C2 is 64x too small) scores such
# one-level flips as structure: `sun_array_aces13` scores 0.85 there with dither 1.0 (0.96
# without), and at least 0.995 with standard 8 x 8 SSIM either way. With dither 1.0, the other
# eleven scenes score 0.926 to 0.998 there (measured in the WP 5.6 fix round).
import bpy
bpy.ops.object.delete()
bpy.data.objects.remove(bpy.data.objects['Light'])
bpy.data.materials.new('Grey')
bpy.data.materials['Grey'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.4, 0.4, 0.4, 1.0)
bpy.data.materials['Grey'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.7
bpy.data.materials.new('Floor')
bpy.data.materials['Floor'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.3, 0.3, 0.32, 1.0)
bpy.data.materials['Floor'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.9
bpy.ops.mesh.primitive_plane_add(size=20.0, location=(0.0, 0.0, 0.0))
bpy.context.object.data.materials.append(bpy.data.materials['Floor'])
bpy.ops.mesh.primitive_uv_sphere_add(radius=0.8, location=(-1.2, 0.5, 0.8))
bpy.ops.object.shade_smooth()
bpy.context.object.data.materials.append(bpy.data.materials['Grey'])
bpy.ops.mesh.primitive_cylinder_add(vertices=48, radius=0.6, depth=1.4, location=(1.0, 0.8, 0.7))
bpy.ops.object.shade_smooth()
bpy.context.object.data.materials.append(bpy.data.materials['Grey'])
bpy.ops.object.light_add(type='AREA', location=(2.0, -2.0, 4.0), rotation=(0.6, 0.0, 0.8))
bpy.context.object.data.shape = 'DISK'
bpy.context.object.data.size = 2.0
bpy.context.object.data.energy = 60.0
bpy.context.object.data.color = (1.0, 0.85, 0.7)
bpy.data.worlds['World'].node_tree.nodes['Background'].inputs['Color'].default_value = (0.1, 0.13, 0.25, 1.0)
bpy.data.worlds['World'].node_tree.nodes['Background'].inputs['Strength'].default_value = 0.15
bpy.data.objects['Camera'].location = (0.0, -6.5, 2.4)
bpy.data.objects['Camera'].rotation_euler = (1.3, 0.0, 0.0)
bpy.context.scene.render.engine = 'BLENDER_EEVEE'
bpy.context.scene.render.resolution_x = 320
bpy.context.scene.render.resolution_y = 180
bpy.context.scene.eevee.taa_render_samples = 16
bpy.context.scene.view_settings.view_transform = 'Standard'
bpy.context.scene.view_settings.exposure = -1.0
bpy.context.scene.render.filepath = 'out/night_low_contrast_dither'
#@render bpy.ops.render.render(write_still=True)
