# WP 1.23: `export_format='GLTF_SEPARATE'`: a `.gltf` JSON with its `.bin` and the texture file
# next to it; the `.glb` extension of the path becomes `.gltf`, as the add-on does.
# The harness copies only the `.gltf` into the fixtures; Blender's `gltf_separate.bin` and
# `gltf_separate_grid.png` are committed next to it (re-copy them when re-blessing).
# golden: startup=factory tier=T2 compare=glb:out/sep/gltf_separate.gltf glb_structure
import bpy
bpy.data.images.new('gltf_separate_grid', 4, 4)
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Material'].node_tree.nodes['Image Texture'].image = bpy.data.images['gltf_separate_grid']
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.ops.export_scene.gltf(filepath='out/sep/gltf_separate.glb', export_format='GLTF_SEPARATE')
