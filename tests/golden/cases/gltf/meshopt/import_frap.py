# WP 7.1: Blender imports frap's compressed files as frap does. The inputs
# (`import_frap.inputs/`) are the files frap writes in the CLI transcript
# `tests/cli/m7/meshopt/export` (a cube with a shape key, a UV sphere, a mesh with loose edges;
# regenerate them by running `frap --verbosity minimal < tests/cli/m7/meshopt/export.stdin` in an
# empty directory and copying ext.glb, khr.glb, ext_sep.gltf/.bin, khr_sep.gltf/.bin, quant.glb
# and gltfpacked/pack.glb as packed.glb): EXT_meshopt_compression and KHR_meshopt_compression
# (GLB and GLTF_SEPARATE), frap_mesh_quantization (KHR_mesh_quantization: UNSIGNED_SHORT
# positions with the dequantization on the node, normalized BYTE normals, normalized
# UNSIGNED_SHORT UVs) and export_use_gltfpack with quantization (one position transform for the
# file, vertex-cache order). Both tools import the same files.
# golden: startup=empty order=canonical tier=T1 tol=1e-5 compare=objects,mesh:Cube,mesh:Sphere,mesh:Edges,mesh:Cube.001,mesh:Sphere.001,mesh:Edges.001,mesh:Cube.002,mesh:Sphere.002,mesh:Edges.002,mesh:Cube.003,mesh:Sphere.003,mesh:Edges.003,mesh:Cube.004,mesh:Sphere.004,mesh:Edges.004,mesh:Cube.005,mesh:Sphere.005,mesh:Edges.005
import bpy
bpy.ops.import_scene.gltf(filepath='ext.glb')
bpy.ops.import_scene.gltf(filepath='khr.glb')
bpy.ops.import_scene.gltf(filepath='ext_sep.gltf')
bpy.ops.import_scene.gltf(filepath='khr_sep.gltf')
bpy.ops.import_scene.gltf(filepath='quant.glb')
bpy.ops.import_scene.gltf(filepath='packed.glb')
