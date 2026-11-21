# WP 3.6: an extension texture's texCoord takes part in the material cache key. The
# transmission texture reads the 'Second' UV map: index 1 on the Cube (UVMap, Second), index 0
# on the plane (only 'Second'), so the material is written twice (get_final_material).
# golden: startup=factory tier=T2 compare=glb:out/transmission_uv.glb glb_structure
import bpy
bpy.data.images.new('Frost', 4, 4)
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Material'].node_tree.nodes['Image Texture'].image = bpy.data.images['Frost']
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeUVMap')
bpy.data.materials['Material'].node_tree.nodes['UV Map'].uv_map = 'Second'
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['UV Map'].outputs[0], bpy.data.materials['Material'].node_tree.nodes['Image Texture'].inputs[0])
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Transmission Weight'])
bpy.data.meshes['Cube'].uv_layers.new(name='Second')
bpy.ops.mesh.primitive_plane_add(location=(3, 0, 0))
bpy.context.object.data.uv_layers['UVMap'].name = 'Second'
bpy.context.object.data.materials.append(bpy.data.materials['Material'])
bpy.ops.export_scene.gltf(filepath='out/transmission_uv.glb')
