# WP 1.19: mesh.primitive_torus_add at T1 (a Python operator in Blender: object_data_add). Defaults
# at a moved 3D cursor; segments and radii with a location; EXT_INT mode without UVs and a large
# rotation (decomposed from matrix_world); align='VIEW' alone (no rotation: the WORLD preference
# wins); align='VIEW' with a rotation (the stale factory view matrix); align='CURSOR' with a
# rotation (the cursor rotation).
# golden: startup=factory compare=objects,context,mesh:Torus,mesh:Torus.001,mesh:Torus.002,mesh:Torus.003,mesh:Torus.004,mesh:Torus.005 tier=T1
import bpy
bpy.context.scene.cursor.location = (1, 2, 3)
bpy.context.scene.cursor.rotation_euler = (0.2, 0.3, 0.4)
bpy.ops.mesh.primitive_torus_add()
bpy.ops.mesh.primitive_torus_add(major_segments=12, minor_segments=6, major_radius=2, minor_radius=0.5, location=(0, 0, 0))
bpy.ops.mesh.primitive_torus_add(major_segments=8, minor_segments=4, mode='EXT_INT', abso_major_rad=1.5, abso_minor_rad=0.5, generate_uvs=False, rotation=(4, 5, 6))
bpy.ops.mesh.primitive_torus_add(major_segments=6, minor_segments=3, align='VIEW')
bpy.ops.mesh.primitive_torus_add(major_segments=6, minor_segments=3, align='VIEW', rotation=(0.1, 0, 0))
bpy.ops.mesh.primitive_torus_add(major_segments=5, minor_segments=4, align='CURSOR', rotation=(0.1, 0, 0))
