# M8 stabilization round 1: MIRROR without bisect after merge_normals of a cylinder at the origin
# (every vertex merges with its mirror copy).
# golden: startup=factory compare=evaluated:Cylinder tier=T2 tol=0.0001
import bpy
bpy.ops.mesh.primitive_cylinder_add(vertices=12, depth=1.69)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.merge_normals()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.object.modifier_add(type='MIRROR')
bpy.data.objects['Cylinder'].modifiers['Mirror'].use_bisect_axis[0] = False
