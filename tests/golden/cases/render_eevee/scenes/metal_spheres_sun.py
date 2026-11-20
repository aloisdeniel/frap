# golden: startup=factory compare=objects,materials,context tier=T0
# render-golden: image=out/metal_spheres_sun.png
# M5 exit scene (WP 5.6): five metallic spheres (gold, copper, silver, iron, violet tints) from
# roughness 0.05 to 0.95 on a floor, a SUN light of strength 3 with its default angle, a blue-grey
# world colour (ambient and reflections), Standard view transform.
# Dither off (`render.dither_intensity = 0`); `night_low_contrast_dither.py` says why.
import bpy
bpy.ops.object.delete()
bpy.data.objects.remove(bpy.data.objects['Light'])
bpy.ops.mesh.primitive_uv_sphere_add(segments=48, ring_count=24, radius=0.65, location=(-3.0, 0.0, 0.65))
bpy.ops.object.shade_smooth()
bpy.data.materials.new('Metal0')
bpy.data.materials['Metal0'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (1.0, 0.77, 0.34, 1.0)
bpy.data.materials['Metal0'].node_tree.nodes['Principled BSDF'].inputs['Metallic'].default_value = 1.0
bpy.data.materials['Metal0'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.05
bpy.context.object.data.materials.append(bpy.data.materials['Metal0'])
bpy.ops.mesh.primitive_uv_sphere_add(segments=48, ring_count=24, radius=0.65, location=(-1.5, 0.0, 0.65))
bpy.ops.object.shade_smooth()
bpy.data.materials.new('Metal1')
bpy.data.materials['Metal1'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.95, 0.64, 0.54, 1.0)
bpy.data.materials['Metal1'].node_tree.nodes['Principled BSDF'].inputs['Metallic'].default_value = 1.0
bpy.data.materials['Metal1'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.25
bpy.context.object.data.materials.append(bpy.data.materials['Metal1'])
bpy.ops.mesh.primitive_uv_sphere_add(segments=48, ring_count=24, radius=0.65, location=(0.0, 0.0, 0.65))
bpy.ops.object.shade_smooth()
bpy.data.materials.new('Metal2')
bpy.data.materials['Metal2'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.91, 0.92, 0.92, 1.0)
bpy.data.materials['Metal2'].node_tree.nodes['Principled BSDF'].inputs['Metallic'].default_value = 1.0
bpy.data.materials['Metal2'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.45
bpy.context.object.data.materials.append(bpy.data.materials['Metal2'])
bpy.ops.mesh.primitive_uv_sphere_add(segments=48, ring_count=24, radius=0.65, location=(1.5, 0.0, 0.65))
bpy.ops.object.shade_smooth()
bpy.data.materials.new('Metal3')
bpy.data.materials['Metal3'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.56, 0.57, 0.58, 1.0)
bpy.data.materials['Metal3'].node_tree.nodes['Principled BSDF'].inputs['Metallic'].default_value = 1.0
bpy.data.materials['Metal3'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.7
bpy.context.object.data.materials.append(bpy.data.materials['Metal3'])
bpy.ops.mesh.primitive_uv_sphere_add(segments=48, ring_count=24, radius=0.65, location=(3.0, 0.0, 0.65))
bpy.ops.object.shade_smooth()
bpy.data.materials.new('Metal4')
bpy.data.materials['Metal4'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.66, 0.5, 0.82, 1.0)
bpy.data.materials['Metal4'].node_tree.nodes['Principled BSDF'].inputs['Metallic'].default_value = 1.0
bpy.data.materials['Metal4'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.95
bpy.context.object.data.materials.append(bpy.data.materials['Metal4'])
bpy.data.materials.new('Floor')
bpy.data.materials['Floor'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.35, 0.35, 0.35, 1.0)
bpy.data.materials['Floor'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.6
bpy.ops.mesh.primitive_plane_add(size=20.0, location=(0.0, 0.0, 0.0))
bpy.context.object.data.materials.append(bpy.data.materials['Floor'])
bpy.ops.object.light_add(type='SUN', location=(0.0, 0.0, 5.0), rotation=(0.6, 0.2, 0.8))
bpy.context.object.data.energy = 3.0
bpy.data.worlds['World'].node_tree.nodes['Background'].inputs['Color'].default_value = (0.2, 0.25, 0.35, 1.0)
bpy.data.worlds['World'].node_tree.nodes['Background'].inputs['Strength'].default_value = 1.0
bpy.data.objects['Camera'].location = (0.0, -8.5, 2.6)
bpy.data.objects['Camera'].rotation_euler = (1.32, 0.0, 0.0)
bpy.context.scene.render.engine = 'BLENDER_EEVEE'
bpy.context.scene.render.resolution_x = 320
bpy.context.scene.render.resolution_y = 180
bpy.context.scene.eevee.taa_render_samples = 16
bpy.context.scene.view_settings.view_transform = 'Standard'
bpy.context.scene.render.dither_intensity = 0.0
bpy.context.scene.render.filepath = 'out/metal_spheres_sun'
#@render bpy.ops.render.render(write_still=True)
