# WP 1.26: unknown operator keywords (code OP_ARG_UNKNOWN, TypeError) with Blender's doubled
# colon; the first unknown keyword is named. The valid call at the end shows that the failed
# ones left no object behind.
# golden: startup=factory tier=T0 compare=objects
import bpy
bpy.ops.mesh.primitive_cube_add(sizee=2)
bpy.ops.mesh.primitive_cube_add(size=2, siz=3)
bpy.ops.mesh.primitive_cube_add(radius=1)
bpy.ops.mesh.primitive_uv_sphere_add(vertices=8)
bpy.ops.object.select_all(mode='SELECT')
bpy.ops.object.delete(use_globals=True)
bpy.ops.object.mode_set(Mode='OBJECT')
bpy.ops.export_scene.gltf(file_path='out/x.glb')
bpy.ops.mesh.primitive_cube_add(size=1)
