# WP 8.12: `export_unused_images` corner cases. With `export_keep_originals`, a texture written
# from its original file (`ExportImage.from_original`) does not mark its image exported
# (`exported_images` is filled by `encode_image.py` only), so the unused-images pass writes
# the used images again; images it writes are never shared with another (`paint` and
# `paint-1`: two images with the same pixels and name). An image whose file is missing has no
# buffer (`Image.channels` 0): the add-on writes its fill-less encode, a 1x1 white pixel named
# `Image`, as PNG (`AUTO` with a .png path), JPEG (a .jpg path, or `JPEG`), or without a MIME
# type (`WEBP`, `NONE`: the data stays PNG). (GLB files: the fixtures keep no sidecar files;
# `GLTF_SEPARATE` writes `Image.png`, `Image-1.png` the same way, verified on 5.2.4.)
# golden: startup=factory tier=T2 compare=glb:out/keep.glb,glb:out/auto.glb,glb:out/jpeg.glb,glb:out/none.glb,glb:out/webp.glb glb_structure
import bpy
bpy.data.images.new('paint', 4, 2, alpha=True)
bpy.data.images['paint'].pixels = [0.0, 0.1, 0.2, 1.0, 0.3, 0.4, 0.5, 0.5, 0.6, 0.7, 0.8, 1.0, 0.9, 1.0, 0.0, 0.25, 1.0, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 0.5, 0.5, 0.5, 0.0]
bpy.data.images['paint'].filepath_raw = 'out/tex/paint.png'
bpy.data.images['paint'].file_format = 'PNG'
bpy.data.images['paint'].save()
bpy.data.images.load('out/tex/paint.png')
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Material'].node_tree.nodes['Image Texture'].image = bpy.data.images['paint.png']
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.ops.export_scene.gltf(filepath='out/keep.glb', export_keep_originals=True, export_unused_images=True)
bpy.data.images.remove(bpy.data.images['paint'])
bpy.data.images.remove(bpy.data.images['paint.png'])
bpy.data.images.new('Lost', 4, 4)
bpy.data.images['Lost'].source = 'FILE'
bpy.data.images['Lost'].filepath = 'out/nowhere/lost.png'
bpy.data.images.new('LostJ', 4, 4)
bpy.data.images['LostJ'].source = 'FILE'
bpy.data.images['LostJ'].filepath = 'out/nowhere/lost.jpg'
bpy.ops.export_scene.gltf(filepath='out/auto.glb', export_unused_images=True)
bpy.ops.export_scene.gltf(filepath='out/jpeg.glb', export_unused_images=True, export_image_format='JPEG')
bpy.ops.export_scene.gltf(filepath='out/none.glb', export_unused_images=True, export_image_format='NONE')
bpy.ops.export_scene.gltf(filepath='out/webp.glb', export_unused_images=True, export_image_format='WEBP')
