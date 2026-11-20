# golden: startup=factory compare=objects,materials,context tier=T0
# render-golden: image=out/area_lights_aces20.png
# M5 exit scene (WP 5.6): ico spheres and a cube lit by a RECTANGLE area light (1 m x 0.3 m) and
# a DISK area light (0.5 m), soft shadows, black world, ACES 2.0 view transform. The lights stay
# small: frap's penumbras of large lights are wider and darker near contact (D-125).
# Dither off (`render.dither_intensity = 0`); `night_low_contrast_dither.py` says why.
import bpy
bpy.ops.object.delete()
bpy.data.objects.remove(bpy.data.objects['Light'])
bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=3, radius=0.6, location=(-1.4, 0.0, 0.6))
bpy.data.materials.new('Blue')
bpy.data.materials['Blue'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.1, 0.2, 0.7, 1.0)
bpy.data.materials['Blue'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.35
bpy.context.object.data.materials.append(bpy.data.materials['Blue'])
bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=1, radius=0.6, location=(1.4, 0.3, 0.6))
bpy.data.materials.new('Yellow')
bpy.data.materials['Yellow'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.8, 0.65, 0.1, 1.0)
bpy.data.materials['Yellow'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.5
bpy.context.object.data.materials.append(bpy.data.materials['Yellow'])
bpy.ops.mesh.primitive_cube_add(size=0.9, location=(0.0, 1.2, 0.45), rotation=(0.0, 0.0, 0.6))
bpy.data.materials.new('Grey')
bpy.data.materials['Grey'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.5, 0.5, 0.5, 1.0)
bpy.data.materials['Grey'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.3
bpy.context.object.data.materials.append(bpy.data.materials['Grey'])
bpy.data.materials.new('Floor')
bpy.data.materials['Floor'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.7, 0.7, 0.7, 1.0)
bpy.data.materials['Floor'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.8
bpy.ops.mesh.primitive_plane_add(size=20.0, location=(0.0, 0.0, 0.0))
bpy.context.object.data.materials.append(bpy.data.materials['Floor'])
bpy.ops.object.light_add(type='AREA', location=(0.0, -1.0, 3.0), rotation=(0.3, 0.0, 0.0))
bpy.context.object.data.energy = 400.0
bpy.context.object.data.shape = 'RECTANGLE'
bpy.context.object.data.size = 1.0
bpy.context.object.data.size_y = 0.3
bpy.ops.object.light_add(type='AREA', location=(2.5, 1.0, 2.0), rotation=(0.0, 0.9, 0.0))
bpy.context.object.data.energy = 150.0
bpy.context.object.data.color = (1.0, 0.8, 0.6)
bpy.context.object.data.shape = 'DISK'
bpy.context.object.data.size = 0.5
bpy.data.worlds['World'].node_tree.nodes['Background'].inputs['Strength'].default_value = 0.0
bpy.context.scene.render.engine = 'BLENDER_EEVEE'
bpy.context.scene.render.resolution_x = 320
bpy.context.scene.render.resolution_y = 180
bpy.context.scene.eevee.taa_render_samples = 64
bpy.context.scene.view_settings.view_transform = 'ACES 2.0'
bpy.context.scene.render.dither_intensity = 0.0
bpy.context.scene.render.filepath = 'out/area_lights_aces20'
#@render bpy.ops.render.render(write_still=True)
