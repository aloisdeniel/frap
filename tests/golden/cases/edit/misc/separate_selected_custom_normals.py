# M8 stabilization round 1: separate(type='SELECTED') of two faces of the cube with custom normals.
# golden: startup=factory compare=mesh:Cube,mesh:Cube.001 tier=T2 tol=0.0001
import bpy
bpy.ops.mesh.customdata_custom_splitnormals_add()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[0, 1])
bpy.ops.mesh.separate(type='SELECTED')
