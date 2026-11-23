# M8 stabilization round 1 (fuzz, seed 5151): mesh.vertices read in Edit Mode while a key block
# other than the basis is edited shows the reference key's coordinates, as Blender's
# update_from_editmode (bm_to_mesh_shape) writes them; the edit goes to the active block.
# golden: startup=factory compare=mesh:Cube tier=T2 tol=0.0001
import bpy
bpy.ops.object.shape_key_add()
bpy.ops.object.shape_key_add()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.transform.translate(value=(0.5, 0, 0))
bpy.ops.mesh.primitive_cube_add(size=0.5, location=(3, 0, 0))
