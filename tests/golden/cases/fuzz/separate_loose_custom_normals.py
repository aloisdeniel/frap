# M8 stabilization round 1 (minimised fuzz case, seed 0x64e7d90d53df1c5a): mesh.separate carries
# custom split normals as vectors and re-encodes them in each part's corner spaces.
# golden: startup=factory compare=mesh:Cube.001 tier=T2 tol=0.0001
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.primitive_cylinder_add(vertices=7, radius=0.86, depth=1.92, end_fill_type='TRIFAN', location=(0.84, 0.24, -0.62))
bpy.ops.mesh.merge_normals()
bpy.ops.mesh.separate(type='LOOSE')
