# M8 stabilization round 1: mesh.separate with custom split normals (SELECTED and MATERIAL); the
# vectors travel with the corners and are re-encoded in the corner spaces of each part.
# golden: startup=factory compare=mesh:Cube,mesh:Cube.001 tier=T2 tol=0.0001
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.primitive_cylinder_add(vertices=7, location=(3, 0, 0))
bpy.ops.mesh.faces_shade_smooth()
bpy.ops.mesh.average_normals()
bpy.ops.mesh.separate(type='MATERIAL')
bpy.ops.mesh.separate(type='SELECTED')
