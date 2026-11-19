# WP 3.9: a vertex-painted low-poly terrain. A subdivided grid gets a BYTE_COLOR corner colour
# (painted per corner) and a FLOAT_COLOR point colour; its material multiplies an image's colour
# by the corner colour (Color Attribute node named 'Paint' -> COLOR_0 from that layer, the
# point colour follows as COLOR_1). export_vertex_color='ACTIVE' writes the render colour first,
# 'NONE' none. Round trip (§17.2) of the three files: the importer creates colour attributes
# from COLOR_n and a Color Attribute node.
# golden: startup=factory tier=T2 compare=glb:out/terrain.glb,glb:out/terrain_active.glb,glb:out/terrain_none.glb,glb:out/terrain_rt.glb,glb:out/terrain_active_rt.glb,glb:out/terrain_none_rt.glb
import bpy
bpy.data.objects.remove(bpy.data.objects['Cube'])
bpy.ops.mesh.primitive_grid_add(x_subdivisions=3, y_subdivisions=3, size=4)
bpy.context.object.name = 'Terrain'
bpy.data.objects['Terrain'].data.materials.append(bpy.data.materials['Material'])
bpy.data.meshes['Grid'].color_attributes.new('Paint', 'BYTE_COLOR', 'CORNER')
bpy.data.meshes['Grid'].color_attributes.new('Height', 'FLOAT_COLOR', 'POINT')
bpy.data.meshes['Grid'].color_attributes['Paint'].data[0].color = (0.2, 0.6, 0.1, 1.0)
bpy.data.meshes['Grid'].color_attributes['Paint'].data[1].color = (0.5, 0.4, 0.2, 1.0)
bpy.data.meshes['Grid'].color_attributes['Paint'].data[5].color = (0.9, 0.9, 0.95, 1.0)
bpy.data.meshes['Grid'].color_attributes['Paint'].data[9].color = (0.1, 0.3, 0.8, 1.0)
bpy.data.meshes['Grid'].color_attributes['Paint'].data[14].color = (0.6, 0.2, 0.2, 1.0)
bpy.data.meshes['Grid'].color_attributes['Height'].data[4].color = (1.0, 0.0, 0.0, 1.0)
bpy.data.meshes['Grid'].color_attributes['Height'].data[7].color = (0.0, 0.5, 1.0, 1.0)
bpy.data.meshes['Grid'].color_attributes.active_color_index = 1
bpy.data.meshes['Grid'].color_attributes.render_color_index = 1
bpy.data.images.new('Soil', 2, 2)
bpy.data.images['Soil'].pixels = [0.6, 0.5, 0.4, 1.0, 0.7, 0.6, 0.5, 1.0, 0.5, 0.45, 0.35, 1.0, 0.65, 0.55, 0.45, 1.0]
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Material'].node_tree.nodes['Image Texture'].image = bpy.data.images['Soil']
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeVertexColor')
bpy.data.materials['Material'].node_tree.nodes['Color Attribute'].layer_name = 'Paint'
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeMix')
bpy.data.materials['Material'].node_tree.nodes['Mix'].data_type = 'RGBA'
bpy.data.materials['Material'].node_tree.nodes['Mix'].blend_type = 'MULTIPLY'
bpy.data.materials['Material'].node_tree.nodes['Mix'].inputs['Factor'].default_value = 1.0
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Mix'].inputs[6])
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Color Attribute'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Mix'].inputs[7])
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Mix'].outputs[2], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.ops.export_scene.gltf(filepath='out/terrain.glb')
bpy.ops.export_scene.gltf(filepath='out/terrain_active.glb', export_vertex_color='ACTIVE')
bpy.ops.export_scene.gltf(filepath='out/terrain_none.glb', export_vertex_color='NONE')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/terrain.glb')
bpy.ops.export_scene.gltf(filepath='out/terrain_rt.glb')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/terrain_active.glb')
bpy.ops.export_scene.gltf(filepath='out/terrain_active_rt.glb', export_vertex_color='ACTIVE')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/terrain_none.glb')
bpy.ops.export_scene.gltf(filepath='out/terrain_none_rt.glb')
