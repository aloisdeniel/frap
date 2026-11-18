# WP 2.9: Mesh.flip_normals (corner data follows its vertex), Mesh.set_sharp_from_angle
# (sharp_edge/sharp_face reset), Mesh.split_faces (sharp edges and flat faces split; new
# vertices and edges appended), uv_layers.remove (the active/render UV maps move to the
# clamped index; unknown names raise) and vertex_colors.new (BYTE_COLOR on CORNER, white,
# do_init copies the active colour). (Edit Mode: frap refuses them, D-06.)
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Cylinder,mesh:Grid tier=T1
import bpy
bpy.data.meshes['Cube'].flip_normals()
bpy.data.meshes['Cube'].vertex_colors.new()
bpy.data.meshes['Cube'].attributes['Col'].data[1].color = (1.0, 0.5, 0.25, 1.0)
bpy.data.meshes['Cube'].vertex_colors.new(name='Col')
bpy.data.meshes['Cube'].vertex_colors.new(name='X', do_init=False)
bpy.data.meshes['Cube'].uv_layers.new(name='Second')
bpy.data.meshes['Cube'].uv_layers.new(name='Third')
bpy.data.meshes['Cube'].uv_layers.active = bpy.data.meshes['Cube'].uv_layers['Third']
bpy.data.meshes['Cube'].uv_layers.remove(bpy.data.meshes['Cube'].uv_layers['Third'])
bpy.data.objects['Cube'].name = bpy.data.meshes['Cube'].uv_layers.active.name
bpy.data.meshes['Cube'].uv_layers.remove(bpy.data.meshes['Cube'].uv_layers[0])
bpy.data.meshes['Cube'].set_sharp_from_angle(angle=0.5)
bpy.ops.mesh.primitive_cylinder_add(vertices=8, location=(3, 0, 0))
bpy.data.meshes['Cylinder'].set_sharp_from_angle(angle=1.0)
bpy.data.meshes['Cylinder'].split_faces()
bpy.ops.mesh.primitive_grid_add(x_subdivisions=3, y_subdivisions=3, location=(0, 3, 0))
bpy.data.meshes['Grid'].set_sharp_from_angle(angle=0.0)
bpy.data.meshes['Grid'].polygons[0].use_smooth = True
bpy.data.meshes['Grid'].polygons[3].use_smooth = True
bpy.data.meshes['Grid'].polygons[4].use_smooth = True
bpy.data.meshes['Grid'].split_faces()
bpy.data.meshes['Grid'].set_sharp_from_angle()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.object.mode_set(mode='OBJECT')
