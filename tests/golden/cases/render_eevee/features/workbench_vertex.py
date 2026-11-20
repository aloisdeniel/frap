# golden: startup=factory compare=objects tier=T0
# render-golden: image=out/workbench_vertex.png min_ssim=0.95
# Workbench colour type VERTEX (WP 5.5): each object draws its active colour attribute (a
# painted BYTE_COLOR corner colour on a grid, a FLOAT_COLOR point colour on the cube); the
# sphere has none and draws its material colour.
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
bpy.data.meshes['Cube'].color_attributes.new('Height', 'FLOAT_COLOR', 'POINT')
bpy.data.meshes['Cube'].color_attributes['Height'].data[0].color = (1.0, 0.0, 0.0, 1.0)
bpy.data.meshes['Cube'].color_attributes['Height'].data[1].color = (0.0, 1.0, 0.0, 1.0)
bpy.data.meshes['Cube'].color_attributes['Height'].data[2].color = (0.0, 0.0, 1.0, 1.0)
bpy.data.meshes['Cube'].color_attributes['Height'].data[3].color = (1.0, 1.0, 0.0, 1.0)
bpy.data.meshes['Cube'].color_attributes['Height'].data[4].color = (0.0, 1.0, 1.0, 1.0)
bpy.data.meshes['Cube'].color_attributes['Height'].data[5].color = (1.0, 0.0, 1.0, 1.0)
bpy.data.meshes['Cube'].color_attributes['Height'].data[6].color = (1.0, 1.0, 1.0, 1.0)
bpy.data.meshes['Cube'].color_attributes['Height'].data[7].color = (0.1, 0.1, 0.1, 1.0)
bpy.ops.mesh.primitive_uv_sphere_add(segments=24, ring_count=12, radius=0.6, location=(0.0, 1.8, 0.6))
bpy.data.materials.new('Teal')
bpy.data.materials['Teal'].diffuse_color = (0.1, 0.6, 0.5, 1.0)
bpy.context.object.data.materials.append(bpy.data.materials['Teal'])
bpy.data.objects['Camera'].location = (0.0, -7.0, 5.0)
bpy.data.objects['Camera'].rotation_euler = (0.95, 0.0, 0.0)
bpy.context.scene.render.engine = 'BLENDER_WORKBENCH'
bpy.context.scene.display.shading.color_type = 'VERTEX'
bpy.context.scene.render.resolution_x = 200
bpy.context.scene.render.resolution_y = 120
bpy.context.scene.view_settings.view_transform = 'Standard'
bpy.context.scene.render.filepath = 'out/workbench_vertex'
#@render bpy.ops.render.render(write_still=True)
