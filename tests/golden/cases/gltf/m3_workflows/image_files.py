# WP 3.9: textures from image files. An agent paints a 4x2 RGBA image, saves it as PNG
# (Image.save), loads it back with bpy.data.images.load (the PNG bytes are passed through
# unchanged, the glTF image named after the file without extension) and packs a second loaded
# copy (Image.pack: the same bytes and name, so both textures share one glTF image; its alpha
# makes that material BLEND); a third material uses image.new through the operator with a
# generated colour. Round trip (§17.2): the importer creates packed images from the GLB, whose
# bytes the re-export passes through again.
# golden: startup=factory tier=T2 compare=glb:out/files.glb,glb:out/files_rt.glb glb_structure
import bpy
bpy.data.images.new('Paint', 4, 2, alpha=True)
bpy.data.images['Paint'].pixels = [0.0, 0.1, 0.2, 1.0, 0.3, 0.4, 0.5, 0.5, 0.6, 0.7, 0.8, 1.0, 0.9, 1.0, 0.0, 0.25, 1.0, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 0.5, 0.5, 0.5, 0.0]
bpy.data.images['Paint'].filepath_raw = 'out/paint.png'
bpy.data.images['Paint'].file_format = 'PNG'
bpy.data.images['Paint'].save()
bpy.data.images.load('out/paint.png')
bpy.data.images.load('out/paint.png', check_existing=False)
bpy.data.images['paint.png.001'].pack()
bpy.ops.image.new(name='Flat', width=4, height=4, color=(0.2, 0.4, 0.8, 1.0))
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Material'].node_tree.nodes['Image Texture'].image = bpy.data.images['paint.png']
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.data.meshes['Cube'].materials.append(bpy.data.materials.new('Packed'))
bpy.data.materials['Packed'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Packed'].node_tree.nodes['Image Texture'].image = bpy.data.images['paint.png.001']
bpy.data.materials['Packed'].node_tree.links.new(bpy.data.materials['Packed'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Packed'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.data.materials['Packed'].node_tree.links.new(bpy.data.materials['Packed'].node_tree.nodes['Image Texture'].outputs['Alpha'], bpy.data.materials['Packed'].node_tree.nodes['Principled BSDF'].inputs['Alpha'])
bpy.data.meshes['Cube'].materials.append(bpy.data.materials.new('Flat'))
bpy.data.materials['Flat'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Flat'].node_tree.nodes['Image Texture'].image = bpy.data.images['Flat']
bpy.data.materials['Flat'].node_tree.links.new(bpy.data.materials['Flat'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Flat'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.data.meshes['Cube'].polygons[1].material_index = 1
bpy.data.meshes['Cube'].polygons[2].material_index = 2
bpy.ops.export_scene.gltf(filepath='out/files.glb')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/files.glb')
bpy.ops.export_scene.gltf(filepath='out/files_rt.glb')
