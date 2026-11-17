# WP 1.26: operator arguments of the wrong type (code OP_ARG_TYPE, TypeError): float, int, bool,
# string and enum properties, vector items, None, and a scalar for a vector (Blender's message
# body is empty then). Ints are accepted for float properties, 0/1 for booleans.
# golden: startup=factory tier=T0 compare=objects
import bpy
bpy.ops.mesh.primitive_cube_add(size='x')
bpy.ops.mesh.primitive_cube_add(size=None)
bpy.ops.mesh.primitive_cube_add(enter_editmode='yes')
bpy.ops.mesh.primitive_circle_add(vertices=1.5)
bpy.ops.mesh.primitive_circle_add(vertices='8')
bpy.ops.object.select_all(action=1)
bpy.ops.mesh.primitive_cube_add(align=None)
bpy.ops.mesh.primitive_cube_add(location=1)
bpy.ops.mesh.primitive_cube_add(location=('a', 0, 0))
bpy.ops.mesh.primitive_cube_add(rotation=(0, None, 0))
bpy.ops.object.mode_set(mode='OBJECT', toggle=2)
bpy.ops.export_scene.gltf(filepath=1)
bpy.ops.mesh.primitive_cube_add(size=3, location=(1, 0, 0), calc_uvs=0)
