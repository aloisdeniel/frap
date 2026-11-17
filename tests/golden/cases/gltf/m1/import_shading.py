# WP 1.23: import_shading='NORMALS' detects smooth and flat faces from the exported normals;
# merge_vertices=True welds the corner-split vertices.
# Each tool imports its own export, whose vertex order may legitimately differ
# (glTF outputs compare canonically), so meshes compare canonically.
# golden: order=canonical startup=empty tier=T1 compare=objects,mesh:Sphere,mesh:Sphere.001 tol=1e-4
import bpy
bpy.ops.mesh.primitive_uv_sphere_add(segments=8, ring_count=5)
bpy.context.object.data.shade_smooth()
bpy.ops.export_scene.gltf(filepath='out/sphere.glb')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/sphere.glb')
bpy.ops.import_scene.gltf(filepath='out/sphere.glb', merge_vertices=True)
