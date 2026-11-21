# WP 7.2: export_draco_mesh_compression_enable with the add-on's defaults (level 6, 14/10/12/
# 10/12 bits) on the factory cube and a linked duplicate (the shared primitive is encoded
# once), a smooth UV sphere with two UV maps, a mesh with two material slots (two primitives)
# and a smooth mesh with non-manifold (bowtie) vertices, which EdgeBreaker splits (13 encoded
# points for 10 vertices): the extension on every TRIANGLES primitive (bufferView, attribute
# ids in attribute order), explicit mode 4, accessors without bufferView with the encoded
# counts and the original POSITION bounds, the extension used and required. Then levels 0 and
# 10 (sequential), lossless and coarse quantizations, the CORE backend, and the same scene
# without Draco (`plain.glb`: the input of the byte-identity tests in frap-io-gltf
# `post/draco.rs`). The defaults and the lossless file are imported back: every lossless mesh
# is compared, of the defaults only the cube (axis-aligned normals): Blender's Draco decoder
# fuses multiply-adds, and the importer's 1/50000 rounding of normals turns that last bit into
# other custom normals (deviation D-132); the decoded values are checked within 1e-6 against
# Blender's decoder in frap-io-gltf `import/decompress/draco.rs`.
# golden: startup=factory tier=T2 tol=1e-5 order=canonical compare=glb:out/default.glb,glb:out/level0.glb,glb:out/level10.glb,glb:out/lossless.glb,glb:out/coarse.glb,glb:out/core.glb,glb:out/plain.glb,mesh:Cube.001,mesh:Cube.002,mesh:Sphere.002,mesh:Two.002,mesh:Bow.002 glb_structure
import bpy
bpy.ops.mesh.primitive_uv_sphere_add(segments=12, ring_count=6, radius=1.0, location=(3, 0, 0))
bpy.ops.object.shade_smooth()
bpy.data.meshes['Sphere'].uv_layers.new(name='Second')
bpy.data.meshes.new('Two')
bpy.data.meshes['Two'].from_pydata([(0, 0, 0), (1, 0, 0), (1, 1, 0), (0, 1, 0), (2, 0, 0.5), (2, 1, 0.5), (3, 0, 0), (3, 1, 0.2)], [], [(0, 1, 2, 3), (1, 4, 5, 2), (4, 6, 7, 5)])
bpy.data.meshes['Two'].materials.append(bpy.data.materials['Material'])
bpy.data.materials.new('Other')
bpy.data.meshes['Two'].materials.append(bpy.data.materials['Other'])
bpy.data.meshes['Two'].polygons[1].material_index = 1
bpy.data.objects.new('Two', bpy.data.meshes['Two'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Two'])
bpy.data.objects['Two'].location = (-4, 0, 0)
bpy.data.meshes.new('Bow')
bpy.data.meshes['Bow'].from_pydata([(0, 0, 0), (1, 0, 0), (1, 1, 0), (-1, 0, 0), (-1, -1, 0), (0, 2, 0), (2, 2, 1), (0, -1, 1), (2, 1, 1), (3, 0, 0)], [], [(0, 1, 2), (0, 3, 4), (5, 6, 2), (0, 4, 7), (2, 8, 9)])
bpy.data.meshes['Bow'].shade_smooth()
bpy.data.objects.new('Bow', bpy.data.meshes['Bow'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Bow'])
bpy.data.objects['Bow'].location = (0, -4, 0)
bpy.data.objects.new('Twin', bpy.data.meshes['Cube'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Twin'])
bpy.data.objects['Twin'].location = (0, 3, 0)
bpy.ops.export_scene.gltf(filepath='out/default.glb', export_draco_mesh_compression_enable=True)
bpy.ops.export_scene.gltf(filepath='out/level0.glb', export_draco_mesh_compression_enable=True, export_draco_mesh_compression_level=0)
bpy.ops.export_scene.gltf(filepath='out/level10.glb', export_draco_mesh_compression_enable=True, export_draco_mesh_compression_level=10)
bpy.ops.export_scene.gltf(filepath='out/lossless.glb', export_draco_mesh_compression_enable=True, export_draco_position_quantization=0, export_draco_normal_quantization=0, export_draco_texcoord_quantization=0)
bpy.ops.export_scene.gltf(filepath='out/coarse.glb', export_draco_mesh_compression_enable=True, export_draco_mesh_compression_level=3, export_draco_position_quantization=8, export_draco_normal_quantization=6, export_draco_texcoord_quantization=7)
#@frap bpy.ops.export_scene.gltf(filepath='out/core.glb', export_draco_mesh_compression_enable=True, frap_draco_backend='CORE')
#@blender bpy.ops.export_scene.gltf(filepath='out/core.glb', export_draco_mesh_compression_enable=True)
bpy.ops.export_scene.gltf(filepath='out/plain.glb')
bpy.ops.import_scene.gltf(filepath='out/default.glb')
bpy.ops.import_scene.gltf(filepath='out/lossless.glb')
