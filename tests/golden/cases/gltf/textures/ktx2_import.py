# WP 7.3: a frap-written KHR_texture_basisu file with PNG fallbacks (`ktx2_fallback.glb`,
# `export_scene.gltf(frap_texture_format='KTX2')`, see ktx2_import.inputs/) imports in Blender:
# the add-on does not know the extension and uses each texture's core `source`, and frap
# makes the same choice, so the images, materials and objects are equal. A file whose textures
# have only their KTX2 source (`ktx2_only.gltf`, KHR_texture_basisu required) is refused by
# Blender ("Extension KHR_texture_basisu is not available on this addon version", deviation
# D-133) and imported by frap (tests/cli/m7/textures/ktx2.toml). The re-export writes the images
# as PNG (AUTO): the KTX2 extension is frap's, not the add-on's.
# golden: startup=empty tier=T2 compare=objects,materials,glb:out/again.glb
import bpy
bpy.ops.import_scene.gltf(filepath='ktx2_fallback.glb')
bpy.data.images['Grid'].size[0]
bpy.data.images['Grid'].file_format
bpy.data.images['Rough'].colorspace_settings.name
bpy.ops.export_scene.gltf(filepath='out/again.glb')
