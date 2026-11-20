# golden: startup=factory compare=objects,materials,context tier=T0
# render-golden: image=out/suzanne_subsurf_filmic.png
# M5 exit scene (WP 5.6): Suzanne with a level-2 Subdivision Surface modifier and smooth shading
# (evaluated mesh, smooth normals) in a red glossy material on a grey floor, lit by the factory
# point light, Filmic view transform.
# Dither off (`render.dither_intensity = 0`): two renders' 8-bit dither noise is uncorrelated.
import bpy
bpy.ops.object.delete()
bpy.ops.mesh.primitive_monkey_add(size=2.0, location=(0.0, 0.0, 0.0))
bpy.ops.object.shade_smooth()
bpy.ops.object.modifier_add(type='SUBSURF')
bpy.context.object.modifiers['Subdivision'].levels = 2
bpy.context.object.modifiers['Subdivision'].render_levels = 2
bpy.data.materials.new('Red')
bpy.data.materials['Red'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.6, 0.04, 0.03, 1.0)
bpy.data.materials['Red'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.3
bpy.context.object.data.materials.append(bpy.data.materials['Red'])
bpy.data.materials.new('Floor')
bpy.data.materials['Floor'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.5, 0.5, 0.5, 1.0)
bpy.data.materials['Floor'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.8
bpy.ops.mesh.primitive_plane_add(size=20.0, location=(0.0, 0.0, -1.0))
bpy.context.object.data.materials.append(bpy.data.materials['Floor'])
bpy.context.scene.render.engine = 'BLENDER_EEVEE'
bpy.context.scene.render.resolution_x = 320
bpy.context.scene.render.resolution_y = 180
bpy.context.scene.eevee.taa_render_samples = 16
bpy.context.scene.view_settings.view_transform = 'Filmic'
bpy.context.scene.render.dither_intensity = 0.0
bpy.context.scene.render.filepath = 'out/suzanne_subsurf_filmic'
#@render bpy.ops.render.render(write_still=True)
