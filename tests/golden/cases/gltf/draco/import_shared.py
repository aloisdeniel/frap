# WP 7.2: Draco primitives sharing an accessor. The input is Blender 5.2.4's export of two
# 4x3 grids: 'A' has a shape key, so it is encoded sequentially (preserve_triangle_order), 'B'
# with EdgeBreaker (its first vertex raised); both primitives use the same view-less index
# accessor, which decodes to different indices for each. The add-on decodes and reads each
# primitive in turn, so each mesh gets its own triangles; frap must too (it decodes every
# primitive first: a later decode writes a copy of the shared accessor).
# golden: startup=empty tier=T2 tol=1e-5 order=canonical compare=objects,mesh:A,mesh:B
import bpy
bpy.ops.import_scene.gltf(filepath='blender_shared.glb')
