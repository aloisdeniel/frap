# golden: startup=factory compare=objects,materials,context tier=T0
# render-golden: image=out/ortho_exposure_look.png
# M5 exit scene (WP 5.6): an orthographic camera (ortho_scale 9) over a torus, a cylinder, a cone
# and an ico sphere in four colours, a sun and a point light, AgX with the "AgX - High Contrast"
# look, exposure 0.5 and gamma 1.1.
# Dither off (`render.dither_intensity = 0`); `night_low_contrast_dither.py` says why.
import bpy
bpy.ops.object.delete()
bpy.data.objects.remove(bpy.data.objects['Light'])
bpy.ops.mesh.primitive_torus_add(location=(-1.5, -1.5, 0.25))
bpy.ops.object.shade_smooth()
bpy.data.materials.new('Magenta')
bpy.data.materials['Magenta'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.6, 0.05, 0.4, 1.0)
bpy.data.materials['Magenta'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.3
bpy.context.object.data.materials.append(bpy.data.materials['Magenta'])
bpy.ops.mesh.primitive_cylinder_add(radius=0.7, depth=1.5, location=(1.5, -1.5, 0.75))
bpy.data.materials.new('Lime')
bpy.data.materials['Lime'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.3, 0.7, 0.05, 1.0)
bpy.data.materials['Lime'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.5
bpy.context.object.data.materials.append(bpy.data.materials['Lime'])
bpy.ops.mesh.primitive_cone_add(radius1=0.8, depth=1.8, location=(-1.5, 1.5, 0.9))
bpy.data.materials.new('Cyan')
bpy.data.materials['Cyan'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.05, 0.5, 0.7, 1.0)
bpy.data.materials['Cyan'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.4
bpy.context.object.data.materials.append(bpy.data.materials['Cyan'])
bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=2, radius=0.8, location=(1.5, 1.5, 0.8))
bpy.data.materials.new('Sand')
bpy.data.materials['Sand'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.75, 0.6, 0.4, 1.0)
bpy.data.materials['Sand'].node_tree.nodes['Principled BSDF'].inputs['Metallic'].default_value = 0.5
bpy.data.materials['Sand'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.35
bpy.context.object.data.materials.append(bpy.data.materials['Sand'])
bpy.data.materials.new('Floor')
bpy.data.materials['Floor'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.5, 0.5, 0.55, 1.0)
bpy.data.materials['Floor'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.9
bpy.ops.mesh.primitive_plane_add(size=20.0, location=(0.0, 0.0, 0.0))
bpy.context.object.data.materials.append(bpy.data.materials['Floor'])
bpy.ops.object.light_add(type='SUN', location=(0.0, 0.0, 5.0), rotation=(0.7, 0.3, -0.5))
bpy.context.object.data.energy = 2.5
bpy.ops.object.light_add(type='POINT', location=(0.0, 0.0, 2.5))
bpy.context.object.data.energy = 200.0
bpy.context.object.data.color = (1.0, 0.85, 0.7)
bpy.data.worlds['World'].node_tree.nodes['Background'].inputs['Color'].default_value = (0.05, 0.05, 0.05, 1.0)
bpy.data.worlds['World'].node_tree.nodes['Background'].inputs['Strength'].default_value = 1.0
bpy.data.objects['Camera'].location = (7.0, -7.0, 7.0)
bpy.data.objects['Camera'].rotation_euler = (0.9553166, 0.0, 0.7853982)
bpy.data.cameras['Camera'].type = 'ORTHO'
bpy.data.cameras['Camera'].ortho_scale = 9.0
bpy.context.scene.render.engine = 'BLENDER_EEVEE'
bpy.context.scene.render.resolution_x = 320
bpy.context.scene.render.resolution_y = 180
bpy.context.scene.eevee.taa_render_samples = 16
bpy.context.scene.view_settings.view_transform = 'AgX'
bpy.context.scene.view_settings.look = 'AgX - High Contrast'
bpy.context.scene.view_settings.exposure = 0.5
bpy.context.scene.view_settings.gamma = 1.1
bpy.context.scene.render.dither_intensity = 0.0
bpy.context.scene.render.filepath = 'out/ortho_exposure_look'
#@render bpy.ops.render.render(write_still=True)
