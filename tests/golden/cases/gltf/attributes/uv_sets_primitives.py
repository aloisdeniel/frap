# WP 3.7: multiple UV maps (every map becomes TEXCOORD_n, in uv_layers order, whichever is
# active) and multi-primitive meshes (one primitive per used material slot, in slot order;
# an empty slot exports a primitive without material; a material index past the last slot uses
# the last one), with the NAME colour on every primitive (also the one without material), and
# one primitive for the whole mesh with export_materials='NONE'.
# golden: startup=factory tier=T2 compare=glb:out/uvs.glb,glb:out/prims.glb,glb:out/prims_none.glb glb_structure
import bpy
bpy.data.meshes['Cube'].uv_layers.new(name='Second')
bpy.data.meshes['Cube'].uv_layers.new(name='Third')
bpy.data.meshes['Cube'].uv_layers['Second'].data[0].uv = (0.25, 0.75)
bpy.data.meshes['Cube'].uv_layers['Third'].data[3].uv = (2.0, -1.0)
bpy.data.meshes['Cube'].uv_layers.active_index = 2
bpy.ops.export_scene.gltf(filepath='out/uvs.glb')
bpy.data.materials.new('Red')
bpy.data.materials['Red'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (1, 0, 0, 1)
bpy.data.meshes['Cube'].materials.append(bpy.data.materials['Red'])
bpy.data.meshes['Cube'].materials.append(None)
bpy.data.meshes['Cube'].polygons[1].material_index = 1
bpy.data.meshes['Cube'].polygons[2].material_index = 2
bpy.data.meshes['Cube'].polygons[3].material_index = 1
bpy.data.meshes['Cube'].polygons[4].material_index = 5
bpy.data.meshes['Cube'].color_attributes.new('Col', 'BYTE_COLOR', 'POINT')
bpy.data.meshes['Cube'].color_attributes['Col'].data[0].color = (0.0, 1.0, 0.0, 1.0)
bpy.ops.export_scene.gltf(filepath='out/prims.glb', export_vertex_color='NAME', export_vertex_color_name='Col')
bpy.ops.export_scene.gltf(filepath='out/prims_none.glb', export_materials='NONE')
