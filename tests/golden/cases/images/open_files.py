# WP 3.2: `image.open` with `directory` + `files`: one image per file, consecutive frame numbers
# grouped into one SEQUENCE image (`use_sequence_detection`), an existing image reused.
# golden: startup=factory tier=T0 compare=objects
import bpy
bpy.data.images.new('S', 2, 1)
bpy.data.images['S'].save(filepath='out/seq_001.png', save_copy=True)
bpy.data.images['S'].save(filepath='out/seq_002.png', save_copy=True)
bpy.data.images['S'].save(filepath='out/other.png', save_copy=True)
bpy.ops.image.open(directory='out/', files=[{'name': 'seq_002.png'}, {'name': 'seq_001.png'}, {'name': 'other.png'}])
bpy.data.objects['Cube'].name = bpy.data.images['seq_001.png'].source
bpy.data.objects['Camera'].name = bpy.data.images['other.png'].source
bpy.data.objects['Light'].name = bpy.data.images['seq_001.png'].filepath
bpy.ops.image.open(directory='out/', files=[{'name': 'other.png'}], use_sequence_detection=False)
bpy.data.images['other.png.001']
bpy.ops.image.open(directory='out/', files=[{'name': 'seq_001.png'}, {'name': 'seq_002.png'}], use_sequence_detection=False)
bpy.data.objects.new('Seq', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Seq'])
bpy.data.objects['Seq'].name = bpy.data.images['seq_002.png'].source
bpy.ops.image.open(directory='out/', files=[{'name': 'nope.png'}])
