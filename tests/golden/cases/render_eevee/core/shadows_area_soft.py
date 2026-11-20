# golden: startup=factory compare=objects,materials,context tier=T0
# render-golden: image=out/shadows_area_soft.exr probes=74,64;64,76;100,64;64,30;20,20 tol=0.08
# EEVEE core golden (WP 5.1): a diffuse plane (base 0.8, roughness 1, Specular IOR Level 0), black world,
# top orthographic camera, linear EXR; soft shadow: the cube under a 1 m SQUARE AREA light 3 m up with `use_shadow_jitter` (penumbra from shadow origins jittered over the light, as frap always renders it).
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
bpy.ops.mesh.primitive_cube_add(size=1.0, location=(0.0, 0.0, 1.5))
bpy.context.object.data.materials.append(bpy.data.materials['Material'])
bpy.ops.object.light_add(type='AREA', location=(0.0, 0.0, 3.0))
bpy.context.object.data.energy = 300.0
bpy.context.object.data.use_shadow_jitter = True
bpy.context.object.data.shadow_jitter_overblur = 0.0
bpy.context.scene.render.engine = 'BLENDER_EEVEE'
bpy.context.scene.render.resolution_x = 128
bpy.context.scene.render.resolution_y = 128
bpy.context.scene.view_settings.view_transform = 'Standard'
bpy.context.scene.render.image_settings.file_format = 'OPEN_EXR'
bpy.context.scene.render.filepath = 'out/shadows_area_soft'
#@render bpy.ops.render.render(write_still=True)
