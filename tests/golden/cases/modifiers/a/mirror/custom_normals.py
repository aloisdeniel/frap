# M8 stabilization round 1 (minimised fuzz case, seed 5151): MIRROR mirrors custom split normals
# (the first half's corner normals through the normal matrix, encoded in the mirrored corners'
# spaces); frap used to keep the source's encoded values.
# golden: startup=factory compare=evaluated:Cylinder,mesh:Cylinder tier=T2 tol=0.0001
import bpy
bpy.ops.mesh.primitive_cylinder_add(vertices=12, depth=1.69, location=(0.08, 0.95, -1.45))
bpy.ops.object.modifier_add(type='MIRROR')
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.merge_normals()
bpy.ops.object.mode_set(mode='OBJECT')
