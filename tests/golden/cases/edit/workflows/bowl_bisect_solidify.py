# WP 2.13 workflow (bowl): an ico sphere cut above its equator with mesh.bisect (the part above
# the plane cleared), given thickness with mesh.solidify and shaded smooth.
# golden: startup=empty compare=objects,context,mesh:Icosphere tier=T2 tol=1e-4
import bpy
bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=2, radius=0.5, location=(0, 0, 0.5))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.bisect(plane_co=(0, 0, 0.6), plane_no=(0, 0, 1), clear_outer=True)
bpy.data.meshes['Icosphere'].total_vert_sel
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.solidify(thickness=0.03)
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.faces_shade_smooth()
