# WP 8.1: pinned re-unwraps with ABF++. Each ico sphere is unwrapped once; then the first gets
# three pinned UVs (uv.pin on selected vertices) and the second a single one, and the second
# ANGLE_BASED unwrap keeps them as LSCM constraints (a pinned island is not rotated or packed;
# one pin: the island keeps its area and is moved onto its pin). Ico spheres, not UV spheres:
# with pins Blender's own result on a UV sphere changes between runs (its element order is
# run-dependent, D-19).
# golden: startup=empty compare=objects,context,mesh:Icosphere,mesh:Icosphere.001 tier=T2 tol=1e-5
import bpy
bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=2)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.seams_from_islands()
bpy.ops.uv.unwrap(method='ANGLE_BASED')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[3, 20, 40])
bpy.ops.uv.pin()
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.unwrap(method='ANGLE_BASED')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=2, location=(3, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.seams_from_islands()
bpy.ops.uv.unwrap(method='ANGLE_BASED')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[25])
bpy.ops.uv.pin()
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.unwrap(method='ANGLE_BASED')
bpy.ops.object.mode_set(mode='OBJECT')
