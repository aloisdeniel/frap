# WP 7.2: Blender imports frap's Draco. The inputs are frap's exports of the CLI transcript
# `tests/cli/m7/draco/draco_bpy` (its `.out` files, byte for byte; a test in frap-io-gltf
# `import/decompress/draco.rs` keeps the copies equal): `frap_default.glb` (the defaults: a
# smooth sphere, a coloured cube, a mesh with two primitives, a grid with a morph target, so a
# sequential primitive) and `frap_lossless.glb` (level 10, lossless positions, normals and
# UVs). Both tools import both files: the objects, every lossless mesh, and of the defaults
# the cube and the grid (axis-aligned normals; the others: deviation D-132, Draco and fused
# multiply-adds).
# golden: startup=empty tier=T2 tol=1e-5 order=canonical compare=objects,mesh:Cube,mesh:Grid,mesh:Cube.001,mesh:Grid.001,mesh:Sphere.001,mesh:Two.001
import bpy
bpy.ops.import_scene.gltf(filepath='frap_default.glb')
bpy.ops.import_scene.gltf(filepath='frap_lossless.glb')
