# golden: startup=factory compare=objects tier=T0
# render-golden: image=out/vertex_colors_missing.png min_ssim=0.985 max_delta_e=0.3
# EEVEE shading feature golden (WP 5.5): missing colour attributes. A Color Attribute node
# reads a missing attribute as (0, 0, 0, 1) in Blender 5.2.4: the factory cube's node names an
# attribute that does not exist ('Nope', colour and alpha linked) and the grid's node has no
# name on a mesh without colour attributes. Both render black (alpha 1), not white.
import bpy
bpy.data.objects['Cube'].location = (1.6, 0.0, 1.0)
bpy.ops.mesh.primitive_grid_add(x_subdivisions=3, y_subdivisions=3, size=3.0, location=(-1.4, 0.0, 0.0))
bpy.data.materials.new('Unnamed')
bpy.data.materials['Unnamed'].node_tree.nodes.new('ShaderNodeVertexColor')
bpy.data.materials['Unnamed'].node_tree.links.new(bpy.data.materials['Unnamed'].node_tree.nodes['Color Attribute'].outputs['Color'], bpy.data.materials['Unnamed'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.data.objects['Grid'].data.materials.append(bpy.data.materials['Unnamed'])
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeVertexColor')
bpy.data.materials['Material'].node_tree.nodes['Color Attribute'].layer_name = 'Nope'
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Color Attribute'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Color Attribute'].outputs['Alpha'], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Alpha'])
bpy.data.objects['Camera'].location = (0.0, -7.0, 5.0)
bpy.data.objects['Camera'].rotation_euler = (0.95, 0.0, 0.0)
bpy.context.scene.render.engine = 'BLENDER_EEVEE'
bpy.context.scene.render.resolution_x = 200
bpy.context.scene.render.resolution_y = 120
bpy.context.scene.view_settings.view_transform = 'Standard'
bpy.context.scene.eevee.taa_render_samples = 16
bpy.context.scene.render.filepath = 'out/vertex_colors_missing'
#@render bpy.ops.render.render(write_still=True)
