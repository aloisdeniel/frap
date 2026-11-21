# M3+M4 stabilization round 5 (M34S5-4): SUBSURF, WIREFRAME, then DISPLACE (no texture: every
# vertex moves 0.5 along its normal), evaluated and exported with export_apply=True. The
# wireframe's thin-face normals carry SUBSURF's float rounding (after_subsurf.py) into the
# displacement, and the displaced thin faces amplify it again: corner normals up to ~1.2e-4
# from Blender's. Compared at tol=2e-4 (D-122).
# golden: startup=factory compare=evaluated:Torus,glb:out/t.glb tier=T2 tol=2e-4 glb_structure
import bpy
bpy.data.objects.remove(bpy.data.objects['Cube'])
bpy.ops.mesh.primitive_torus_add(location=(5, 0, 0))
bpy.ops.object.modifier_add(type='SUBSURF')
bpy.ops.object.modifier_add(type='WIREFRAME')
bpy.ops.object.modifier_add(type='DISPLACE')
bpy.ops.export_scene.gltf(filepath='out/t.glb', export_apply=True)
