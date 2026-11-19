# M3+M4 stabilization round 2 (D-120): DECIMATE COLLAPSE after SUBSURF. The subdivided
# positions are T2 (within ~1e-7 of Blender's), and the collapse of a symmetric mesh breaks
# its many equal-cost ties by those last bits, so frap collapses other edges than Blender:
# the result is checked at T3 (closed manifold of the same genus, face count in range).
# golden: startup=factory compare=evaluated:Cube tier=T3 check=manifold,euler:2,faces:20-60
import bpy
bpy.data.objects['Cube'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Cube'].modifiers['Subdivision'].levels = 2
bpy.data.objects['Cube'].modifiers.new(name='Decimate', type='DECIMATE')
bpy.data.objects['Cube'].modifiers['Decimate'].ratio = 0.3
