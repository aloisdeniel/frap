# WP 1.23: flat faces split their corners, smooth faces share vertex normals, and a mix of both
# in one mesh (Mesh.shade_smooth / shade_flat and per-polygon use_smooth).
# golden: startup=factory tier=T2 compare=glb:out/shading.glb tol=1e-4 glb_structure
import bpy
bpy.ops.mesh.primitive_uv_sphere_add(segments=8, ring_count=5, location=(3, 0, 0))
bpy.context.object.data.shade_smooth()
bpy.ops.mesh.primitive_cylinder_add(vertices=8, location=(-3, 0, 0))
bpy.context.object.data.shade_smooth()
bpy.data.meshes['Cylinder'].polygons[0].use_smooth = False
bpy.data.meshes['Cylinder'].polygons[1].use_smooth = False
bpy.data.meshes['Cube'].shade_flat()
bpy.ops.export_scene.gltf(filepath='out/shading.glb')
