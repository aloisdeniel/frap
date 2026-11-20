# golden: startup=factory compare=objects,materials,context tier=T0
# render-golden: image=out/interior_room_wide.png
# M5 exit scene (WP 5.6): a camera with an 18 mm lens inside a room (a scaled cube seen from
# inside, EEVEE draws back faces), a warm point light under the ceiling (radius 0.3 m) casting
# shadows of a table-like box and a sphere on the walls and floor, Filmic view transform.
# Dither off (`render.dither_intensity = 0`): two renders' 8-bit dither noise is uncorrelated.
import bpy
bpy.ops.object.delete()
bpy.data.objects.remove(bpy.data.objects['Light'])
bpy.ops.mesh.primitive_cube_add(size=1.0, location=(0.0, 0.0, 1.5), scale=(8.0, 8.0, 3.0))
bpy.data.materials.new('Plaster')
bpy.data.materials['Plaster'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.7, 0.68, 0.62, 1.0)
bpy.data.materials['Plaster'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.9
bpy.context.object.data.materials.append(bpy.data.materials['Plaster'])
bpy.ops.mesh.primitive_cube_add(size=1.0, location=(0.5, 1.0, 0.4), scale=(1.6, 0.9, 0.8))
bpy.data.materials.new('Wood')
bpy.data.materials['Wood'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.35, 0.18, 0.08, 1.0)
bpy.data.materials['Wood'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.5
bpy.context.object.data.materials.append(bpy.data.materials['Wood'])
bpy.ops.mesh.primitive_uv_sphere_add(radius=0.5, location=(-1.5, 0.5, 0.5))
bpy.ops.object.shade_smooth()
bpy.data.materials.new('Ceramic')
bpy.data.materials['Ceramic'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.1, 0.3, 0.6, 1.0)
bpy.data.materials['Ceramic'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.15
bpy.context.object.data.materials.append(bpy.data.materials['Ceramic'])
bpy.ops.object.light_add(type='POINT', location=(0.0, 0.5, 2.7))
bpy.context.object.data.energy = 400.0
bpy.context.object.data.color = (1.0, 0.8, 0.6)
bpy.context.object.data.shadow_soft_size = 0.3
bpy.data.worlds['World'].node_tree.nodes['Background'].inputs['Strength'].default_value = 0.0
bpy.data.objects['Camera'].location = (-0.5, -3.6, 1.4)
bpy.data.objects['Camera'].rotation_euler = (1.45, 0.0, -0.12)
bpy.data.cameras['Camera'].lens = 18.0
bpy.context.scene.render.engine = 'BLENDER_EEVEE'
bpy.context.scene.render.resolution_x = 320
bpy.context.scene.render.resolution_y = 180
bpy.context.scene.eevee.taa_render_samples = 16
bpy.context.scene.view_settings.view_transform = 'Filmic'
bpy.context.scene.render.dither_intensity = 0.0
bpy.context.scene.render.filepath = 'out/interior_room_wide'
#@render bpy.ops.render.render(write_still=True)
