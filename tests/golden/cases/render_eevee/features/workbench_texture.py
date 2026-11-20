# golden: startup=factory compare=objects tier=T0
# render-golden: image=out/workbench_texture.png min_ssim=0.95
# Workbench colour type TEXTURE (WP 5.5): each material draws the image of its active texture
# node (the first texture node: none is flagged) on the active UV map. The cube shows a
# generated 64 x 64 UV_GRID image (linear filtering), the plane a 32 x 32 one (Closest, REPEAT,
# its UVs scaled by two so the grid repeats); the sphere's material has no image and draws its
# own colour, and the cone, displayed SOLID, draws its material colour too.
import bpy
bpy.ops.image.new(name='Colors', width=64, height=64, generated_type='UV_GRID')
bpy.ops.image.new(name='Grid', width=32, height=32, generated_type='UV_GRID')
bpy.data.objects['Cube'].location = (1.4, 0.0, 1.0)
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Material'].node_tree.nodes['Image Texture'].image = bpy.data.images['Colors']
bpy.ops.mesh.primitive_plane_add(size=3.0, location=(-1.5, 0.0, 0.0))
bpy.data.materials.new('Floor')
bpy.data.materials['Floor'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Floor'].node_tree.nodes['Image Texture'].image = bpy.data.images['Grid']
bpy.data.materials['Floor'].node_tree.nodes['Image Texture'].interpolation = 'Closest'
bpy.data.materials['Floor'].node_tree.nodes['Image Texture'].extension = 'REPEAT'
bpy.context.object.data.materials.append(bpy.data.materials['Floor'])
bpy.context.object.data.uv_layers['UVMap'].data[0].uv = (0.0, 0.0)
bpy.context.object.data.uv_layers['UVMap'].data[1].uv = (2.0, 0.0)
bpy.context.object.data.uv_layers['UVMap'].data[2].uv = (2.0, 2.0)
bpy.context.object.data.uv_layers['UVMap'].data[3].uv = (0.0, 2.0)
bpy.ops.mesh.primitive_uv_sphere_add(segments=24, ring_count=12, radius=0.6, location=(0.0, 1.8, 0.6))
bpy.data.materials.new('Teal')
bpy.data.materials['Teal'].diffuse_color = (0.1, 0.6, 0.5, 1.0)
bpy.context.object.data.materials.append(bpy.data.materials['Teal'])
bpy.ops.mesh.primitive_cone_add(radius1=0.6, depth=1.2, location=(0.2, -1.6, 0.6))
bpy.context.object.data.materials.append(bpy.data.materials['Material'])
bpy.context.object.display_type = 'SOLID'
bpy.data.objects['Camera'].location = (0.0, -7.0, 5.0)
bpy.data.objects['Camera'].rotation_euler = (0.95, 0.0, 0.0)
bpy.context.scene.render.engine = 'BLENDER_WORKBENCH'
bpy.context.scene.display.shading.color_type = 'TEXTURE'
bpy.context.scene.render.resolution_x = 200
bpy.context.scene.render.resolution_y = 120
bpy.context.scene.view_settings.view_transform = 'Standard'
bpy.context.scene.render.filepath = 'out/workbench_texture'
#@render bpy.ops.render.render(write_still=True)
