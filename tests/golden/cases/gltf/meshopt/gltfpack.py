# WP 7.1: export_use_gltfpack. Blender runs the external gltfpack (its add-on preference
# gltfpack_path_ui, empty by default, which makes Blender fail after writing the file:
# deviations §1); frap maps the props onto in-process meshoptimizer passes and writes the packed
# copy to `gltfpacked/` next to the file. Here `-noq` is off (quantization with one position
# transform for the file, 16-bit UVs) and `-si 1` (no simplification): the packed file is the
# scene within the quantization error. Blender's side writes the unpacked scene at the packed
# path. After the import, applying the transforms gives world-space meshes that agree within
# tol 1e-3 (14 bits of the file's extent); gltfpack reorders vertices for the vertex cache, so
# meshes compare canonically. Normals are left out as in `quantization`.
# golden: startup=factory order=canonical tier=T2 tol=1e-3 compare=mesh:Cube,mesh:Sphere
import bpy
bpy.ops.mesh.primitive_uv_sphere_add(segments=12, ring_count=6, location=(3.0, 1.0, 0.5))
#@frap    bpy.ops.export_scene.gltf(filepath='out/p.glb', export_normals=False, export_use_gltfpack=True, export_gltfpack_noq=False, export_gltfpack_tc=False)
#@blender bpy.ops.export_scene.gltf(filepath='out/gltfpacked/p.glb', export_normals=False)
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/gltfpacked/p.glb')
bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
