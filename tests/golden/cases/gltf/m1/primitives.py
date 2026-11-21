# WP 1.23: every mesh primitive with default options, each at its own location.
# golden: startup=factory tier=T2 compare=glb:out/primitives.glb tol=1e-4 glb_structure
import bpy
bpy.ops.object.select_all(action='SELECT')
bpy.ops.object.delete()
bpy.ops.mesh.primitive_plane_add(location=(0, 0, 0))
bpy.ops.mesh.primitive_cube_add(location=(3, 0, 0))
bpy.ops.mesh.primitive_circle_add(location=(6, 0, 0), fill_type='NGON')
bpy.ops.mesh.primitive_uv_sphere_add(location=(0, 3, 0), segments=12, ring_count=6)
bpy.ops.mesh.primitive_ico_sphere_add(location=(3, 3, 0), subdivisions=2)
bpy.ops.mesh.primitive_cylinder_add(location=(6, 3, 0), vertices=10)
bpy.ops.mesh.primitive_cone_add(location=(0, 6, 0), vertices=9)
bpy.ops.mesh.primitive_torus_add(location=(3, 6, 0), major_segments=12, minor_segments=6)
bpy.ops.mesh.primitive_grid_add(location=(6, 6, 0), x_subdivisions=4, y_subdivisions=3)
bpy.ops.mesh.primitive_monkey_add(location=(0, 9, 0))
bpy.ops.export_scene.gltf(filepath='out/primitives.glb')
