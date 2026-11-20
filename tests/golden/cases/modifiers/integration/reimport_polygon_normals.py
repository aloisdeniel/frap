# M5+M6 W0 (M3+M4 leftover): `MeshPolygon.normal` after importing Blender's `export_apply`
# file (the export_apply_stack fixture, copied as input): Blender's getter computes
# `face_normal_calc` from the positions (triangle/quad special cases, Newell for n-gons), not the
# custom-normal mix that `Mesh.polygon_normals` returns (up to 0.076 apart on the bevelled,
# subdivided cube). Compared through the dumps' `face_normals` (`polygons/normal`).
# golden: startup=factory tier=T1 tol=1e-6 compare=mesh:Cube.001,mesh:Cylinder,mesh:Cone,mesh:Sphere,mesh:Plane,mesh:Plane.001
import bpy
bpy.ops.import_scene.gltf(filepath='stack.glb')
