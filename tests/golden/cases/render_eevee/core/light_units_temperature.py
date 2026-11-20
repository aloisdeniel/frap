# golden: startup=factory compare=objects,materials,context tier=T0
# render-golden: image=out/light_units_temperature.exr probes=32,32;40,32;48,32 tol=0.05 channels=rgb
# EEVEE core golden (WP 5.1): a diffuse plane (base 0.8, roughness 1, Specular IOR Level 0), black world,
# top orthographic camera, linear EXR; a 60 W POINT light with `use_temperature` at 3200 K and `diffuse_factor` 0.5.
# The radiance at the probes (3 x 3 means) must equal Blender 5.2.4's EEVEE within the tolerance.
import bpy
bpy.data.objects.remove(bpy.data.objects['Cube'])
bpy.data.objects.remove(bpy.data.objects['Light'])
bpy.ops.mesh.primitive_plane_add(size=10)
bpy.context.object.data.materials.append(bpy.data.materials['Material'])
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.8, 0.8, 0.8, 1.0)
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Specular IOR Level'].default_value = 0.0
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 1.0
bpy.context.scene.world.node_tree.nodes['Background'].inputs['Strength'].default_value = 0.0
bpy.context.scene.camera.location = (0.0, 0.0, 10.0)
bpy.context.scene.camera.rotation_euler = (0.0, 0.0, 0.0)
bpy.context.scene.camera.data.type = 'ORTHO'
bpy.context.scene.camera.data.ortho_scale = 10.0
bpy.ops.object.light_add(type='POINT', location=(0.0, 0.0, 1.5))
bpy.context.object.data.energy = 60.0
bpy.context.object.data.use_temperature = True
bpy.context.object.data.temperature = 3200.0
bpy.context.object.data.diffuse_factor = 0.5
bpy.context.scene.render.engine = 'BLENDER_EEVEE'
bpy.context.scene.render.resolution_x = 64
bpy.context.scene.render.resolution_y = 64
bpy.context.scene.view_settings.view_transform = 'Standard'
bpy.context.scene.render.image_settings.file_format = 'OPEN_EXR'
bpy.context.scene.render.filepath = 'out/light_units_temperature'
#@render bpy.ops.render.render(write_still=True)
