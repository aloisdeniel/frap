# golden: startup=factory compare=objects,materials,context tier=T0
# render-golden: image=out/spot_stage_punchy.png
# M5 exit scene (WP 5.6): a cone and a torus on a stage lit by a SPOT light (45°, blend 0.3)
# from above and in front, a dim world, AgX with the "AgX - Punchy" look.
# Dither off (`render.dither_intensity = 0`): two renders' 8-bit dither noise is uncorrelated.
import bpy
bpy.ops.object.delete()
bpy.data.objects.remove(bpy.data.objects['Light'])
bpy.ops.mesh.primitive_cone_add(vertices=48, radius1=0.8, depth=1.6, location=(-1.0, 0.3, 0.8))
bpy.data.materials.new('Teal')
bpy.data.materials['Teal'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.05, 0.4, 0.35, 1.0)
bpy.data.materials['Teal'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.4
bpy.context.object.data.materials.append(bpy.data.materials['Teal'])
bpy.ops.mesh.primitive_torus_add(major_radius=0.7, minor_radius=0.25, location=(1.2, -0.2, 0.25))
bpy.ops.object.shade_smooth()
bpy.data.materials.new('Orange')
bpy.data.materials['Orange'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.8, 0.3, 0.05, 1.0)
bpy.data.materials['Orange'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.25
bpy.context.object.data.materials.append(bpy.data.materials['Orange'])
bpy.data.materials.new('Floor')
bpy.data.materials['Floor'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.6, 0.6, 0.6, 1.0)
bpy.data.materials['Floor'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.7
bpy.ops.mesh.primitive_plane_add(size=20.0, location=(0.0, 0.0, 0.0))
bpy.context.object.data.materials.append(bpy.data.materials['Floor'])
bpy.ops.object.light_add(type='SPOT', location=(0.0, -3.0, 5.0), rotation=(0.54, 0.0, 0.0))
bpy.context.object.data.energy = 1500.0
bpy.context.object.data.spot_size = 0.7853982
bpy.context.object.data.spot_blend = 0.3
bpy.context.object.data.shadow_soft_size = 0.2
bpy.data.worlds['World'].node_tree.nodes['Background'].inputs['Strength'].default_value = 0.05
bpy.context.scene.render.engine = 'BLENDER_EEVEE'
bpy.context.scene.render.resolution_x = 320
bpy.context.scene.render.resolution_y = 180
bpy.context.scene.eevee.taa_render_samples = 16
bpy.context.scene.view_settings.view_transform = 'AgX'
bpy.context.scene.view_settings.look = 'AgX - Punchy'
bpy.context.scene.render.dither_intensity = 0.0
bpy.context.scene.render.filepath = 'out/spot_stage_punchy'
#@render bpy.ops.render.render(write_still=True)
