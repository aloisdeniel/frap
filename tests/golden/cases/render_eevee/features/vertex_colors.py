# golden: startup=factory compare=objects tier=T0
# render-golden: image=out/vertex_colors.png min_ssim=0.98 max_delta_e=0.6
# EEVEE shading feature golden (WP 5.5): vertex colours. A subdivided grid with a painted
# BYTE_COLOR corner colour read by a Color Attribute node naming it straight into Base Color,
# and the factory cube with a FLOAT_COLOR point colour, the render (default) colour, read by a
# Color Attribute node without a name and multiplied by an orange tint.
import bpy
bpy.data.objects['Cube'].location = (1.6, 0.0, 1.0)
bpy.ops.mesh.primitive_grid_add(x_subdivisions=3, y_subdivisions=3, size=3.0, location=(-1.4, 0.0, 0.0))
bpy.data.meshes['Grid'].color_attributes.new('Paint', 'BYTE_COLOR', 'CORNER')
bpy.data.meshes['Grid'].color_attributes['Paint'].data[0].color = (0.2, 0.6, 0.1, 1.0)
bpy.data.meshes['Grid'].color_attributes['Paint'].data[1].color = (0.9, 0.4, 0.2, 1.0)
bpy.data.meshes['Grid'].color_attributes['Paint'].data[2].color = (0.1, 0.2, 0.9, 1.0)
bpy.data.meshes['Grid'].color_attributes['Paint'].data[5].color = (0.9, 0.9, 0.95, 1.0)
bpy.data.meshes['Grid'].color_attributes['Paint'].data[9].color = (0.1, 0.3, 0.8, 1.0)
bpy.data.meshes['Grid'].color_attributes['Paint'].data[14].color = (0.6, 0.2, 0.2, 1.0)
bpy.data.meshes['Grid'].color_attributes['Paint'].data[20].color = (1.0, 0.9, 0.1, 1.0)
bpy.data.meshes['Grid'].color_attributes['Paint'].data[33].color = (0.0, 0.8, 0.7, 1.0)
bpy.data.materials.new('Painted')
bpy.data.materials['Painted'].node_tree.nodes.new('ShaderNodeVertexColor')
bpy.data.materials['Painted'].node_tree.nodes['Color Attribute'].layer_name = 'Paint'
bpy.data.materials['Painted'].node_tree.links.new(bpy.data.materials['Painted'].node_tree.nodes['Color Attribute'].outputs['Color'], bpy.data.materials['Painted'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.data.objects['Grid'].data.materials.append(bpy.data.materials['Painted'])
bpy.data.meshes['Cube'].color_attributes.new('Height', 'FLOAT_COLOR', 'POINT')
bpy.data.meshes['Cube'].color_attributes['Height'].data[0].color = (1.0, 0.0, 0.0, 1.0)
bpy.data.meshes['Cube'].color_attributes['Height'].data[1].color = (0.0, 1.0, 0.0, 1.0)
bpy.data.meshes['Cube'].color_attributes['Height'].data[2].color = (0.0, 0.0, 1.0, 1.0)
bpy.data.meshes['Cube'].color_attributes['Height'].data[3].color = (1.0, 1.0, 0.0, 1.0)
bpy.data.meshes['Cube'].color_attributes['Height'].data[4].color = (0.0, 1.0, 1.0, 1.0)
bpy.data.meshes['Cube'].color_attributes['Height'].data[5].color = (1.0, 0.0, 1.0, 1.0)
bpy.data.meshes['Cube'].color_attributes['Height'].data[6].color = (1.0, 1.0, 1.0, 1.0)
bpy.data.meshes['Cube'].color_attributes['Height'].data[7].color = (0.1, 0.1, 0.1, 1.0)
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeVertexColor')
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeMix')
bpy.data.materials['Material'].node_tree.nodes['Mix'].data_type = 'RGBA'
bpy.data.materials['Material'].node_tree.nodes['Mix'].blend_type = 'MULTIPLY'
bpy.data.materials['Material'].node_tree.nodes['Mix'].inputs['Factor'].default_value = 1.0
bpy.data.materials['Material'].node_tree.nodes['Mix'].inputs[6].default_value = (1.0, 0.6, 0.3, 1.0)
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Color Attribute'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Mix'].inputs[7])
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Mix'].outputs[2], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.data.objects['Camera'].location = (0.0, -7.0, 5.0)
bpy.data.objects['Camera'].rotation_euler = (0.95, 0.0, 0.0)
bpy.context.scene.render.engine = 'BLENDER_EEVEE'
bpy.context.scene.render.resolution_x = 200
bpy.context.scene.render.resolution_y = 120
bpy.context.scene.view_settings.view_transform = 'Standard'
bpy.context.scene.eevee.taa_render_samples = 16
bpy.context.scene.render.filepath = 'out/vertex_colors'
#@render bpy.ops.render.render(write_still=True)
