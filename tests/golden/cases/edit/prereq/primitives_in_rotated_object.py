# M8 stabilization round 1: mesh primitives added in Edit Mode of a rotated and scaled object
# take ED_object_new_primitive_matrix of the edit object (eul_to_mat3 with cosf/sinf, invert_m3
# by 1/det): positions equal Blender's bit for bit. Compared at 1e-6 (the cone and cylinder UVs
# are 2e-7 off) and at T2 (the spheres do not reuse freed slots in Blender's order, deviations §2).
# golden: startup=factory compare=mesh:Plane tier=T2 tol=1e-6
import bpy
bpy.ops.mesh.primitive_plane_add(location=(-0.8, -1.68, -0.28), rotation=(-0.39, -0.16, -0.46))
bpy.context.object.scale = (1.3, 0.7, 2.1)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.primitive_monkey_add(size=0.94, location=(1.39, 1.62, 0.4))
bpy.ops.mesh.primitive_ico_sphere_add(radius=1.3, location=(0.3, 1, 2), rotation=(0.2, 0.5, 1))
bpy.ops.mesh.primitive_uv_sphere_add(segments=12, ring_count=6, location=(0.3, 1, 2), rotation=(0.2, 0.5, 1))
bpy.ops.mesh.primitive_cylinder_add(vertices=7, location=(0.3, 1, 2), rotation=(0.2, 0.5, 1))
bpy.ops.mesh.primitive_cone_add(location=(0.3, 1, 2))
bpy.ops.mesh.primitive_cube_add(size=1.5, location=(0.3, 1, 2), rotation=(0.2, 0.5, 1), scale=(1, 2, 3))
bpy.ops.mesh.primitive_circle_add(vertices=6, fill_type='NGON', location=(1.69, -0.83, -0.36))
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=3, location=(0.3, 1, 2), rotation=(0.2, 0.5, 1))
