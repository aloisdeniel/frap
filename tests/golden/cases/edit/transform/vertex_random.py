# WP 2.12: transform.vertex_random with Blender's RandomNumberGenerator: offset, uniform,
# normal alignment, seed; the transformed vertices follow the select mode; a second edit object
# adds the hash of its ID name to the seed.
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Cube.001 tier=T1 tol=1e-5
import bpy
bpy.ops.mesh.primitive_cube_add(location=(3, 0, 0), size=1)
bpy.data.objects['Cube'].select_set(True)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.transform.vertex_random(offset=0.2, seed=3)
bpy.ops.transform.vertex_random(offset=0.1, uniform=0.5, normal=0.7, seed=12)
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[1])
bpy.ops.transform.vertex_random(offset=-0.3, normal=1)
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.transform.vertex_random(offset=0.3)
