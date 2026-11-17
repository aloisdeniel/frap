# WP 1.23: import_shading='FLAT' and 'SMOOTH' override the normals of the file.
# Each tool imports its own export, whose vertex order may legitimately differ
# (glTF outputs compare canonically), so meshes compare canonically.
# golden: order=canonical startup=empty tier=T1 compare=objects,mesh:Cylinder,mesh:Cylinder.001 tol=1e-4
import bpy
bpy.ops.mesh.primitive_cylinder_add(vertices=8)
bpy.ops.export_scene.gltf(filepath='out/cyl.glb')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/cyl.glb', import_shading='SMOOTH')
bpy.ops.import_scene.gltf(filepath='out/cyl.glb', import_shading='FLAT')
