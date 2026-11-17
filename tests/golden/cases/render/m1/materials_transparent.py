# golden: startup=factory compare=objects,context,materials tier=T0
# render-golden: image=out/materials_transparent.png
# Render golden (WP 1.24): Workbench MATERIAL colours from Material.diffuse_color (the factory
# material, and a new one on a second object), film_transparent (RGBA PNG with alpha).
import bpy
bpy.data.materials['Material'].diffuse_color = (0.8, 0.05, 0.05, 1.0)
bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=3, radius=0.7, location=(0.0, 0.0, 1.8))
bpy.data.materials.new('Blue')
bpy.data.materials['Blue'].diffuse_color = (0.1, 0.2, 0.9, 1.0)
bpy.context.object.data.materials.append(bpy.data.materials['Blue'])
bpy.context.scene.render.engine = 'BLENDER_WORKBENCH'
bpy.context.scene.render.film_transparent = True
bpy.context.scene.render.resolution_x = 320
bpy.context.scene.render.resolution_y = 180
bpy.context.scene.render.filepath = 'out/materials_transparent'
#@render bpy.ops.render.render(write_still=True)
