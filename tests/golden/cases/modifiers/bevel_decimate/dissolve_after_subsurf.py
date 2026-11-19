# M3+M4 stabilization round 3 (D-120): DECIMATE DISSOLVE after SUBSURF SIMPLE level 2 on Suzanne.
# The subdivided positions are T2 (within ~1e-6 of Blender's) and the planar dissolve sorts the
# many near-zero face angles of the subdivided flat faces by those last bits, so a few other
# edges dissolve (Blender: 1975/3384/1411, frap 1973/3382/1411): checked at T3.
# golden: startup=empty compare=evaluated:Suzanne tier=T3 check=euler:2,verts:1965-1985,faces:1405-1415
import bpy
bpy.ops.mesh.primitive_monkey_add(calc_uvs=False)
bpy.data.objects['Suzanne'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Suzanne'].modifiers['Subdivision'].subdivision_type = 'SIMPLE'
bpy.data.objects['Suzanne'].modifiers['Subdivision'].levels = 2
bpy.data.objects['Suzanne'].modifiers.new(name='Decimate', type='DECIMATE')
bpy.data.objects['Suzanne'].modifiers['Decimate'].decimate_type = 'DISSOLVE'
