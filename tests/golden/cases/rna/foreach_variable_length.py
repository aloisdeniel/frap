# M8 stabilization round 6: the two-argument `foreach_get` / `foreach_set` of the variable-length
# `MeshPolygon.vertices` size their buffer by the faces' `loop_total` sum (the cube's 24 values,
# a 6-sided cone's 24 in 7 faces), not 3 per face. A shorter buffer raises; a longer one is
# accepted by `foreach_set`, which writes the first values (a longer `foreach_get` buffer is
# left out: Blender fills its extra items with uninitialised memory, D-173). The short
# `foreach_set` runs on the cone, which is not compared (Blender has written part of it, D-173).
# A fixed-size array takes a longer `foreach_set` buffer too (`co`: 27 values, 24 written).
# golden: startup=factory tier=T0 compare=mesh:Cube
import bpy
bpy.data.meshes['Cube'].polygons.foreach_get('vertices', [0] * 24)
bpy.data.meshes['Cube'].polygons.foreach_get('vertices', [0] * 23)
bpy.data.meshes['Cube'].polygons.foreach_get('vertices', [0] * 18)
bpy.data.meshes['Cube'].polygons.foreach_set('vertices', [4, 6, 2, 0, 3, 2, 6, 7, 7, 6, 4, 5, 5, 1, 3, 7, 1, 0, 2, 3, 5, 4, 0, 1])
bpy.data.meshes['Cube'].polygons.foreach_set('vertices', [4, 6, 2, 0, 2, 6, 7, 3, 7, 6, 4, 5, 5, 1, 3, 7, 1, 0, 2, 3, 5, 4, 0, 1, 6])
bpy.data.meshes['Cube'].polygons.foreach_get('vertices', [0] * 24)
bpy.ops.mesh.primitive_cone_add(vertices=6)
bpy.data.meshes['Cone'].polygons.foreach_get('vertices', [0] * 21)
bpy.data.meshes['Cone'].polygons.foreach_get('vertices', [0] * 24)
bpy.data.meshes['Cone'].polygons.foreach_set('vertices', [0, 6, 1, 1, 6, 2, 2, 6, 3, 3, 6, 4, 0, 1, 2, 3, 4, 5, 4, 6, 5, 5, 6, 0])
bpy.data.meshes['Cone'].polygons.foreach_set('vertices', [0] * 21)
bpy.data.meshes['Cube'].vertices.foreach_set('co', [-1, -1, -1, -1, -1, 1, -1, 1, -1, -1, 1, 1, 1, -1, -1, 1, -1, 1, 1, 1, -1, 1, 1, 2, 5, 5, 5])
