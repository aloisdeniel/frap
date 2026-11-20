# golden: startup=factory compare=objects,materials,context tier=T0
# render-golden: image=out/sun_array_aces13.png
# M5 exit scene (WP 5.6): a cube with an Array modifier (5 copies, relative offset 1.8) and a
# Bevel modifier, long shadows from a low SUN light (strength 4, angle 2°), sky-blue world, ACES
# 1.3 view transform.
# Dither off (`render.dither_intensity = 0`): two renders' 8-bit dither noise is uncorrelated.
import bpy
bpy.ops.object.delete()
bpy.data.objects.remove(bpy.data.objects['Light'])
bpy.ops.mesh.primitive_cube_add(size=0.8, location=(-3.0, 0.0, 0.4))
bpy.ops.object.modifier_add(type='ARRAY')
bpy.context.object.modifiers['Array'].count = 5
bpy.context.object.modifiers['Array'].relative_offset_displace = (1.8, 0.0, 0.0)
bpy.ops.object.modifier_add(type='BEVEL')
bpy.context.object.modifiers['Bevel'].width = 0.08
bpy.context.object.modifiers['Bevel'].segments = 3
bpy.data.materials.new('Clay')
bpy.data.materials['Clay'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.7, 0.45, 0.3, 1.0)
bpy.data.materials['Clay'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.6
bpy.context.object.data.materials.append(bpy.data.materials['Clay'])
bpy.data.materials.new('Floor')
bpy.data.materials['Floor'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.45, 0.45, 0.42, 1.0)
bpy.data.materials['Floor'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.9
bpy.ops.mesh.primitive_plane_add(size=20.0, location=(0.0, 0.0, 0.0))
bpy.context.object.data.materials.append(bpy.data.materials['Floor'])
bpy.ops.object.light_add(type='SUN', location=(0.0, 0.0, 5.0), rotation=(1.15, 0.0, 0.5))
bpy.context.object.data.energy = 4.0
bpy.context.object.data.angle = 0.0349066
bpy.data.worlds['World'].node_tree.nodes['Background'].inputs['Color'].default_value = (0.3, 0.45, 0.7, 1.0)
bpy.data.worlds['World'].node_tree.nodes['Background'].inputs['Strength'].default_value = 0.6
bpy.data.objects['Camera'].location = (1.0, -9.0, 4.0)
bpy.data.objects['Camera'].rotation_euler = (1.15, 0.0, 0.1)
bpy.context.scene.render.engine = 'BLENDER_EEVEE'
bpy.context.scene.render.resolution_x = 320
bpy.context.scene.render.resolution_y = 180
bpy.context.scene.eevee.taa_render_samples = 16
bpy.context.scene.view_settings.view_transform = 'ACES 1.3'
bpy.context.scene.render.dither_intensity = 0.0
bpy.context.scene.render.filepath = 'out/sun_array_aces13'
#@render bpy.ops.render.render(write_still=True)
