# WP 8.12: `export_keep_originals`: a texture keeps its image file (`ExportImage.from_original`
# of the last image found): the glTF image is the file referenced by a URI relative to the
# exported file, named after the image datablock, its MIME type from the file (a JPEG stays
# JPEG); the pixels are not re-encoded, and a GLB embeds no copy. A texture whose image has no
# file on disk (generated, packed) has no image. Channel packing (metallic/roughness) is not
# done: the original image is referenced as is.
# golden: startup=factory tier=T2 compare=glb:out/keep.glb,glb:out/plain.glb glb_structure
import bpy
bpy.data.images.new('Paint', 4, 2, alpha=True)
bpy.data.images['Paint'].pixels = [0.0, 0.1, 0.2, 1.0, 0.3, 0.4, 0.5, 0.5, 0.6, 0.7, 0.8, 1.0, 0.9, 1.0, 0.0, 0.25, 1.0, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 0.5, 0.5, 0.5, 0.0]
bpy.data.images['Paint'].filepath_raw = 'out/tex/paint.png'
bpy.data.images['Paint'].file_format = 'PNG'
bpy.data.images['Paint'].save()
bpy.data.images['Paint'].filepath_raw = 'out/tex/paint.jpg'
bpy.data.images['Paint'].file_format = 'JPEG'
bpy.data.images['Paint'].save()
bpy.data.images.load('out/tex/paint.png')
bpy.data.images.load('out/tex/paint.jpg')
bpy.data.images.new('Gen', 2, 2)
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Material'].node_tree.nodes['Image Texture'].image = bpy.data.images['paint.jpg']
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Material'].node_tree.nodes['Image Texture.001'].image = bpy.data.images['paint.png']
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeSeparateColor')
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Image Texture.001'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Separate Color'].inputs['Color'])
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Separate Color'].outputs['Blue'], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Metallic'])
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Material'].node_tree.nodes['Image Texture.002'].image = bpy.data.images['Gen']
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Image Texture.002'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Emission Color'])
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Emission Strength'].default_value = 1.0
bpy.ops.export_scene.gltf(filepath='out/keep.glb', export_keep_originals=True)
bpy.ops.export_scene.gltf(filepath='out/sep/keep.gltf', export_format='GLTF_SEPARATE', export_keep_originals=True)
bpy.ops.export_scene.gltf(filepath='out/plain.glb')
