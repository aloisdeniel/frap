# WP 3.9: a web delivery as separate files. export_format='GLTF_SEPARATE' writes the .gltf JSON,
# its .bin buffer and the textures as PNG files next to it (in a `textures` sub-directory with
# export_texture_dir); GLTF_EMBEDDED (allowed through the add-on preference) puts the buffer
# and the images in data URIs. Round trip (§17.2) of the three files (the importer reads the
# external .bin and images) re-exported as GLB. The harness copies
# only the .gltf files into the fixtures: Blender's crate.bin, crate_tex.bin, Brick.png and
# textures/Brick.png are committed next to them (re-copy them when re-blessing).
# golden: startup=factory tier=T2 compare=glb:out/web/crate.gltf,glb:out/web_tex/crate_tex.gltf,glb:out/embedded.gltf,glb:out/web_rt.glb,glb:out/embedded_rt.glb,glb:out/web_tex_rt.glb glb_structure
import bpy
bpy.data.images.new('Brick', 2, 2)
bpy.data.images['Brick'].pixels = [0.7, 0.3, 0.2, 1.0, 0.6, 0.25, 0.15, 1.0, 0.8, 0.8, 0.75, 1.0, 0.65, 0.3, 0.2, 1.0]
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Material'].node_tree.nodes['Image Texture'].image = bpy.data.images['Brick']
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.ops.mesh.primitive_uv_sphere_add(segments=8, ring_count=4, location=(3, 0, 0))
bpy.ops.export_scene.gltf(filepath='out/web/crate.gltf', export_format='GLTF_SEPARATE')
bpy.ops.export_scene.gltf(filepath='out/web_tex/crate_tex.gltf', export_format='GLTF_SEPARATE', export_texture_dir='textures')
bpy.context.preferences.addons['io_scene_gltf2'].preferences.allow_embedded_format = True
bpy.ops.export_scene.gltf(filepath='out/embedded.gltf', export_format='GLTF_EMBEDDED')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/web/crate.gltf')
bpy.ops.export_scene.gltf(filepath='out/web_rt.glb')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/embedded.gltf')
bpy.ops.export_scene.gltf(filepath='out/embedded_rt.glb')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/web_tex/crate_tex.gltf')
bpy.ops.export_scene.gltf(filepath='out/web_tex_rt.glb')
