# WP 7.1: export_meshopt_compression_enable and export_meshopt_extension, each tool importing its
# own export. A cube with a shape key, a UV sphere and a mesh with loose edges go out with
# EXT_meshopt_compression as a GLB (with tangents) and KHR_meshopt_compression as GLTF_SEPARATE,
# and come back as the add-on's compression leaves them: lossless index and vertex codecs, the
# 12-bit EXPONENTIAL filter on positions, normals and morph targets (decoded identically by both
# tools), loose edges as an INDICES stream. Vertex order may legitimately differ (Blender's own
# triangle order of the sphere changes between runs with tangents), so meshes compare
# canonically.
# golden: startup=factory order=canonical tier=T2 tol=1e-5 compare=objects,mesh:Cube,mesh:Sphere,mesh:Edges,mesh:Cube.001,mesh:Sphere.001,mesh:Edges.001
import bpy
bpy.data.objects['Cube'].shape_key_add(name='Basis', from_mix=False)
bpy.data.objects['Cube'].shape_key_add(name='Lift', from_mix=False)
bpy.data.objects['Cube'].data.shape_keys.key_blocks['Lift'].data[0].co = (1.0, 1.0, 1.5)
bpy.ops.mesh.primitive_uv_sphere_add(segments=12, ring_count=6, location=(3.0, 0.0, 0.0))
bpy.data.objects['Sphere'].rotation_euler = (0.3, 0.0, 0.7)
bpy.data.meshes.new('Edges')
bpy.data.meshes['Edges'].from_pydata([(0, 0, 0), (1, 0, 0), (1, 1, 0), (0, 1, 0), (2, 2, 2), (3, 2, 1)], [(4, 5), (5, 0)], [(0, 1, 2, 3)])
bpy.data.objects.new('Edges', bpy.data.meshes['Edges'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Edges'])
bpy.data.objects['Edges'].location = (0.0, -3.0, 0.0)
bpy.ops.export_scene.gltf(filepath='out/ext.glb', export_meshopt_compression_enable=True, export_tangents=True, use_mesh_edges=True)
bpy.ops.export_scene.gltf(filepath='out/khr/scene.gltf', export_format='GLTF_SEPARATE', export_meshopt_compression_enable=True, export_meshopt_extension='KHR_meshopt_compression', use_mesh_edges=True)
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/ext.glb')
bpy.ops.import_scene.gltf(filepath='out/khr/scene.gltf')
