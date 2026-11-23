# golden: startup=factory compare=objects,context,mesh:Cube tier=T2 tol=0.0001
# fuzz: seed=0xbb23fb0941819619 (minimised, M8 stabilization round 1)
import bpy
bpy.ops.mesh.customdata_custom_splitnormals_add()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.faces_shade_smooth()
bpy.ops.mesh.extrude_faces_indiv()
bpy.ops.mesh.edge_split(type='VERT')
