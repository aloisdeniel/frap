# golden: startup=factory compare=objects,context,materials tier=T0
# render-golden: image=out/render_boolean_materials.png min_ssim=0.97
# Render golden (WP 4.7): a material slot made by a modifier. The cube's Boolean (MANIFOLD
# DIFFERENCE, material_mode TRANSFER) cuts a hole with the hidden cylinder; the evaluated mesh
# appends the cylinder's 'Blue' after the cube's own slot and the hole's faces use it, so the
# render draws two slots although the object has one (BKE_object_material_count_eval).
import bpy
bpy.data.materials['Material'].diffuse_color = (0.8, 0.3, 0.1, 1.0)
bpy.ops.mesh.primitive_cylinder_add(vertices=16, radius=0.6, depth=3, location=(0, 0, 0.3), rotation=(0, 0, 0))
bpy.data.materials.new('Blue')
bpy.data.materials['Blue'].diffuse_color = (0.1, 0.25, 0.9, 1.0)
bpy.data.objects['Cylinder'].data.materials.append(bpy.data.materials['Blue'])
bpy.data.objects['Cylinder'].hide_render = True
bpy.data.objects['Cube'].modifiers.new(name='Boolean', type='BOOLEAN')
bpy.data.objects['Cube'].modifiers['Boolean'].object = bpy.data.objects['Cylinder']
bpy.data.objects['Cube'].modifiers['Boolean'].solver = 'MANIFOLD'
bpy.data.objects['Cube'].modifiers['Boolean'].operation = 'DIFFERENCE'
bpy.data.objects['Cube'].modifiers['Boolean'].material_mode = 'TRANSFER'
bpy.context.scene.render.engine = 'BLENDER_WORKBENCH'
bpy.context.scene.display.shading.color_type = 'MATERIAL'
bpy.context.scene.render.resolution_x = 320
bpy.context.scene.render.resolution_y = 180
bpy.context.scene.render.filepath = 'out/render_boolean_materials'
#@render bpy.ops.render.render(write_still=True)
