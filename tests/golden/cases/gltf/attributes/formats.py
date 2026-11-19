# WP 3.7: export formats. GLB (binary chunk), and GLTF_EMBEDDED (one .gltf with the buffer and
# the texture as data URIs), which the add-on offers only with its allow_embedded_format
# preference; the same scene with a packed texture in both.
# golden: startup=factory tier=T2 compare=glb:out/scene.glb,glb:out/embedded.gltf,glb:out/embedded_noext.gltf
import bpy
bpy.data.images.new('embedded_grid', 4, 4)
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Material'].node_tree.nodes['Image Texture'].image = bpy.data.images['embedded_grid']
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.ops.mesh.primitive_uv_sphere_add(segments=8, ring_count=4, location=(3, 0, 0))
bpy.ops.export_scene.gltf(filepath='out/scene.glb')
bpy.context.preferences.addons['io_scene_gltf2'].preferences.allow_embedded_format = True
bpy.ops.export_scene.gltf(filepath='out/embedded.gltf', export_format='GLTF_EMBEDDED')
bpy.ops.export_scene.gltf(filepath='out/embedded_noext', export_format='GLTF_EMBEDDED')
