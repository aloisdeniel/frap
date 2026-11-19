# M3+M4 stabilization round 5 (M34S5-4): WIREFRAME after SUBSURF on the default torus. WIREFRAME
# alone matches Blender (defaults.py, settings.py), but SUBSURF's Catmull-Clark positions are
# within float rounding (~1e-7, OpenSubdiv's accumulation order; D-120) and the 0.02-thick
# wireframe faces amplify it in their corner normals: 812 corners differ by more than 1e-5,
# at most 3.2e-5 (one face). Compared at tol=5e-5 (D-122).
# golden: startup=factory compare=evaluated:Torus tier=T2 tol=5e-5
import bpy
bpy.ops.mesh.primitive_torus_add(location=(5, 0, 0))
bpy.ops.object.modifier_add(type='SUBSURF')
bpy.ops.object.modifier_add(type='WIREFRAME')
