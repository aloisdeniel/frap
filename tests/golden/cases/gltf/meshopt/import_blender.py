# WP 7.1: Blender-written meshopt files import in frap as in Blender. The inputs
# (`import_blender.inputs/`, written by `tests/cli/m7/meshopt/blender_meshopt.py make` with
# Blender 5.2.4) hold a cube with a shape key, a UV sphere, a mesh with loose edges and a skinned
# cylinder, exported with export_meshopt_compression_enable, export_tangents and use_mesh_edges:
# EXT_meshopt_compression as a GLB (data in buffer 0, the fallback buffer 1) and
# KHR_meshopt_compression as GLTF_SEPARATE (the fallback buffer 0, the data in the .bin), so the
# vertex codec versions 0 and 1, the EXPONENTIAL filter (positions, normals, tangents, morph
# targets), TRIANGLES and INDICES (loose edges) index streams and unfiltered UVs, joints and
# weights are decoded. Both tools import the same files.
# golden: startup=empty tier=T1 tol=1e-5 compare=objects,mesh:Cube,mesh:Sphere,mesh:Edges,mesh:Cylinder,mesh:Cube.001,mesh:Sphere.001,mesh:Edges.001,mesh:Cylinder.001
import bpy
bpy.ops.import_scene.gltf(filepath='ext.glb')
bpy.ops.import_scene.gltf(filepath='khr_sep.gltf')
