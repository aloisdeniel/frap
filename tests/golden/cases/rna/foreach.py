# WP 1.26: bulk access with Blender's two-argument forms: `foreach_get` into a literal list of
# the right length, `foreach_set` from a literal flat list (positions are compared), and a
# `foreach_get` buffer of the wrong length (RuntimeError). A `foreach_set` of the wrong length
# is left out: Blender raises but has already written part of the array, frap writes nothing.
# golden: startup=factory tier=T0 compare=mesh:Cube
import bpy
bpy.data.meshes['Cube'].vertices.foreach_get('co', [0.0] * 24)
bpy.data.meshes['Cube'].polygons.foreach_get('loop_total', [0] * 6)
bpy.data.meshes['Cube'].vertices.foreach_set('co', [0, 0, 0, 1, 0, 0, 0, 1, 0, 1, 1, 0, 0, 0, 1, 1, 0, 1, 0, 1, 1, 1, 1, 2])
bpy.data.meshes['Cube'].vertices.foreach_get('co', [0.0] * 3)
bpy.data.meshes['Cube'].attributes['position'].data.foreach_get('vector', [0.0] * 24)
bpy.data.meshes['Cube'].vertices[7].co.z = 3
