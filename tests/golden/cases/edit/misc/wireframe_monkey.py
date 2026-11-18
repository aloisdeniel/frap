# M2 stabilization round 3 (D-77 family): mesh.wireframe on Suzanne. Positions agree with
# Blender within 1e-6, but the corner normals of the thin wire faces differ by up to ~5e-5:
# 1-ulp position differences are amplified on 6 mm wide quads (Blender's own quad and Newell
# normals differ by 3e-5 there). Above the default T1 1e-5; this pins the tolerance at 1e-4.
# golden: startup=factory compare=objects,context,mesh:Suzanne tier=T1 tol=1e-4
import bpy
bpy.ops.mesh.primitive_monkey_add()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.wireframe()
bpy.ops.object.mode_set(mode='OBJECT')
