# WP 7.1: frap_mesh_quantization (a frap-only option: Blender's exporter quantizes only through
# an external gltfpack). frap writes KHR_mesh_quantization (14-bit UNSIGNED_SHORT positions with
# the dequantization folded into the node, 16-bit normalized UVs); Blender writes the same scene
# unquantized. After the import, applying the transforms gives the world-space meshes, which
# agree within the quantization error (positions within 2^-14 of the extent: tol 1e-3).
# Normals are not exported here: their 8-bit quantization would show in the imported custom
# normals, an encoded attribute no tolerance applies to (the import of a quantized file is
# compared exactly in `import_frap`).
# golden: startup=factory order=canonical tier=T2 tol=1e-3 compare=mesh:Cube,mesh:Sphere
import bpy
bpy.ops.mesh.primitive_uv_sphere_add(segments=12, ring_count=6, location=(3.0, 1.0, 0.5))
bpy.data.objects['Sphere'].scale = (1.5, 1.5, 1.5)
#@frap    bpy.ops.export_scene.gltf(filepath='out/q.glb', export_normals=False, frap_mesh_quantization=True)
#@blender bpy.ops.export_scene.gltf(filepath='out/q.glb', export_normals=False)
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/q.glb')
bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
