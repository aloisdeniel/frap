# WP 3.5 + WP 3.7 (integration): export_vertex_color='MATERIAL' with materials that read colour
# attributes. (a) Base Color reads the render colour (Color Attribute node, no layer name),
# (b) a named POINT colour, (c) a second material reads a named CORNER colour while a third
# reads none, (d) the second material reads another set than the first (glTF cannot express
# it: the later set is not exported), (e) the Color Attribute's Alpha also feeds the Principled
# Alpha (BLEND material: COLOR_0 carries the alpha as a VEC4).
# golden: startup=factory tier=T2 compare=glb:out/a.glb,glb:out/b.glb,glb:out/c.glb,glb:out/d.glb,glb:out/e.glb
import bpy
bpy.data.meshes['Cube'].color_attributes.new('Col', 'BYTE_COLOR', 'CORNER')
bpy.data.meshes['Cube'].color_attributes.new('PCol', 'FLOAT_COLOR', 'POINT')
bpy.data.meshes['Cube'].color_attributes['Col'].data[0].color = (1.0, 0.0, 0.0, 0.5)
bpy.data.meshes['Cube'].color_attributes['PCol'].data[0].color = (0.5, 0.25, 2.0, 0.3)
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeVertexColor')
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Color Attribute'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.ops.export_scene.gltf(filepath='out/a.glb')
bpy.data.materials['Material'].node_tree.nodes['Color Attribute'].layer_name = 'PCol'
bpy.ops.export_scene.gltf(filepath='out/b.glb')
bpy.data.materials.new('Two')
bpy.data.materials['Two'].node_tree.nodes.new('ShaderNodeVertexColor')
bpy.data.materials['Two'].node_tree.nodes['Color Attribute'].layer_name = 'Col'
bpy.data.materials['Two'].node_tree.links.new(bpy.data.materials['Two'].node_tree.nodes['Color Attribute'].outputs['Color'], bpy.data.materials['Two'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.data.materials.new('Three')
bpy.data.meshes['Cube'].materials.append(bpy.data.materials['Two'])
bpy.data.meshes['Cube'].materials.append(bpy.data.materials['Three'])
bpy.data.meshes['Cube'].polygons[1].material_index = 1
bpy.data.meshes['Cube'].polygons[2].material_index = 2
bpy.ops.export_scene.gltf(filepath='out/c.glb')
bpy.data.materials['Two'].node_tree.nodes['Color Attribute'].layer_name = 'PCol'
bpy.ops.export_scene.gltf(filepath='out/d.glb')
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Color Attribute'].outputs['Alpha'], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Alpha'])
bpy.ops.export_scene.gltf(filepath='out/e.glb')
