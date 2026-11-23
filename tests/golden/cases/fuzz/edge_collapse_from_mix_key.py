# M8 stabilization round 1 (minimised fuzz case, seed 5151): edge_collapse while the from_mix key
# is active; the Edit Mode read keeps the reference key's coordinates.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T2 tol=0.0001
import bpy
bpy.ops.object.shape_key_add()
bpy.ops.object.shape_key_add(from_mix=True)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.edge_collapse()
