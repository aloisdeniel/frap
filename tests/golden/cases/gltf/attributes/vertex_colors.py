# WP 3.7: export_vertex_color modes. The factory cube (with a material that reads no colour)
# gets a BYTE_COLOR corner colour and a FLOAT_COLOR point colour: MATERIAL (default) writes the
# fake white COLOR_0 then every colour attribute with alpha; ACTIVE and NAME put the render /
# named colour first; NONE and export_all_vertex_colors=False write none. A plane without a
# material exports its render colour first (export_active_vertex_color_when_no_material), or
# nothing with that option off. export_materials='PLACEHOLDER' counts as no material (render
# colour first), 'VIEWPORT' as a material that reads no colour (fake COLOR_0).
# golden: startup=factory tier=T2 compare=glb:out/default.glb,glb:out/active.glb,glb:out/name.glb,glb:out/none.glb,glb:out/noall.glb,glb:out/nomat.glb,glb:out/nomat_off.glb,glb:out/placeholder.glb,glb:out/viewport.glb glb_structure
import bpy
bpy.data.meshes['Cube'].color_attributes.new('Col', 'BYTE_COLOR', 'CORNER')
bpy.data.meshes['Cube'].color_attributes.new('PCol', 'FLOAT_COLOR', 'POINT')
bpy.data.meshes['Cube'].color_attributes['Col'].data[0].color = (1.0, 0.0, 0.0, 1.0)
bpy.data.meshes['Cube'].color_attributes['Col'].data[5].color = (0.2, 0.4, 0.6, 0.5)
bpy.data.meshes['Cube'].color_attributes['Col'].data[13].color = (0.0, 0.0, 1.0, 0.25)
bpy.data.meshes['Cube'].color_attributes['PCol'].data[0].color = (0.5, 0.25, 2.0, 1.0)
bpy.data.meshes['Cube'].color_attributes['PCol'].data[7].color = (0.0, 1.0, 0.0, 0.0)
bpy.ops.export_scene.gltf(filepath='out/default.glb')
bpy.ops.export_scene.gltf(filepath='out/active.glb', export_vertex_color='ACTIVE')
bpy.ops.export_scene.gltf(filepath='out/name.glb', export_vertex_color='NAME', export_vertex_color_name='PCol')
bpy.ops.export_scene.gltf(filepath='out/none.glb', export_vertex_color='NONE')
bpy.ops.export_scene.gltf(filepath='out/noall.glb', export_all_vertex_colors=False)
bpy.ops.mesh.primitive_plane_add(location=(3, 0, 0))
bpy.context.object.data.color_attributes.new('A', 'FLOAT_COLOR', 'CORNER')
bpy.context.object.data.color_attributes.new('B', 'BYTE_COLOR', 'POINT')
bpy.context.object.data.color_attributes['A'].data[1].color = (0.1, 0.2, 0.3, 0.4)
bpy.context.object.data.color_attributes['B'].data[2].color = (0.9, 0.8, 0.7, 0.6)
bpy.context.object.data.color_attributes.active_color_index = 1
bpy.ops.export_scene.gltf(filepath='out/nomat.glb')
bpy.ops.export_scene.gltf(filepath='out/nomat_off.glb', export_active_vertex_color_when_no_material=False)
bpy.ops.export_scene.gltf(filepath='out/placeholder.glb', export_materials='PLACEHOLDER')
bpy.ops.export_scene.gltf(filepath='out/viewport.glb', export_materials='VIEWPORT')
